mod rto;
mod window;

use super::segment::TcpSegment;
use crate::{prelude::*, protocols::ipv4};
use std::{
    cell::RefCell,
    collections::VecDeque,
    num::{NonZeroU16, Wrapping},
    rc::Rc,
    time::Duration,
};
use window::{TcpReceiveWindow, TcpSendWindow};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TcpConnectionId {
    pub local: ipv4::Endpoint,
    pub remote: ipv4::Endpoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Display)]
pub struct TcpConnectionHandle(NonZeroU16);

impl TcpConnectionHandle {
    // todo: this function should be private to the TCP module.
    pub fn new(n: u16) -> TcpConnectionHandle {
        TcpConnectionHandle(NonZeroU16::new(n).unwrap())
    }
}

impl Into<u16> for TcpConnectionHandle {
    fn into(self) -> u16 {
        self.0.get()
    }
}

pub struct TcpConnection<'a> {
    cxnid: TcpConnectionId,
    handle: TcpConnectionHandle,
    input_queue: Rc<RefCell<VecDeque<TcpSegment>>>,
    local_isn: Wrapping<u32>,
    receive_window: TcpReceiveWindow,
    rt: Runtime<'a>,
    send_window: TcpSendWindow<'a>,
}

impl<'a> TcpConnection<'a> {
    pub fn new(
        cxnid: TcpConnectionId,
        handle: TcpConnectionHandle,
        local_isn: Wrapping<u32>,
        receive_window_size: usize,
        rt: Runtime<'a>,
    ) -> Self {
        let advertised_mss = rt.options().tcp.advertised_mss();
        TcpConnection {
            cxnid,
            handle,
            input_queue: Rc::new(RefCell::new(VecDeque::new())),
            local_isn,
            receive_window: TcpReceiveWindow::new(receive_window_size),
            rt,
            send_window: TcpSendWindow::new(local_isn, advertised_mss),
        }
    }

    pub fn get_handle(&self) -> TcpConnectionHandle {
        self.handle
    }

    pub fn get_cxnid(&self) -> &TcpConnectionId {
        &self.cxnid
    }

    pub fn get_mss(&self) -> usize {
        self.send_window.get_mss()
    }

    pub fn get_last_seq_num(&self) -> Wrapping<u32> {
        self.send_window.get_last_seq_num()
    }

    pub fn negotiate_mss(&mut self, remote_mss: Option<usize>) -> Result<()> {
        self.send_window.negotiate_mss(remote_mss)
    }

    pub fn set_remote_isn(&mut self, value: Wrapping<u32>) {
        self.receive_window.remote_isn(value)
    }

    pub fn incr_seq_num(&mut self) {
        self.send_window.incr_seq_num()
    }

    pub fn try_get_ack_num(&self) -> Option<Wrapping<u32>> {
        self.receive_window.ack_num()
    }

    pub fn get_local_receive_window_size(&self) -> usize {
        self.receive_window.window_size()
    }

    pub fn set_remote_receive_window_size(&mut self, size: usize) {
        self.send_window.set_remote_receive_window_size(size)
    }

    pub fn get_next_transmittable_segment(
        &mut self,
        optional_byte_count: Option<usize>,
    ) -> Option<TcpSegment> {
        trace!("TcpConnection::get_next_transmittable_segment()");
        match self
            .send_window
            .get_next_transmittable_payload(optional_byte_count, self.rt.now())
        {
            None => None,
            Some(payload) => {
                Some(TcpSegment::default().payload(payload).connection(self))
            }
        }
    }

    pub fn get_retransmissions(&mut self) -> Result<VecDeque<TcpSegment>> {
        let options = self.rt.options();
        let mut payloads = self
            .send_window
            .get_retransmissions(self.rt.now(), options.tcp.retries2())?;
        let mut segments = VecDeque::with_capacity(payloads.len());
        while let Some(payload) = payloads.pop_front() {
            segments.push_back(
                TcpSegment::default().payload(payload).connection(self),
            );
        }

        Ok(segments)
    }

    pub fn write(&mut self, bytes: IoVec) {
        self.send_window.push(bytes)
    }

    pub fn read(&mut self) -> IoVec {
        self.receive_window.pop()
    }

    pub fn get_input_queue(&self) -> Rc<RefCell<VecDeque<TcpSegment>>> {
        self.input_queue.clone()
    }

    pub fn acknowledge(&mut self, ack_num: Wrapping<u32>) -> Result<()> {
        self.send_window.acknowledge(ack_num, self.rt.now())
    }

    pub fn receive(
        &mut self,
        segment: TcpSegment,
    ) -> Result<Option<TcpSegment>> {
        let remote_window_size = segment.window_size;
        self.set_remote_receive_window_size(remote_window_size);

        let was_empty = self.receive_window.is_empty();
        self.receive_window.push(segment)?;
        if was_empty && !self.receive_window.is_empty() {
            self.rt.emit_event(Event::TcpBytesAvailable(self.handle));
        }

        if self.receive_window.window_size() == 0 {
            let zero_window = TcpSegment::default().connection(self);
            Ok(Some(zero_window))
        } else {
            Ok(None)
        }
    }

    pub fn get_rto(&self) -> Duration {
        self.send_window.get_rto()
    }
}
