use crate::protocols::{arp, ip, ipv4};
use crate::protocols::tcp2::SeqNumber;
use std::cmp;
use std::convert::TryInto;
use std::future::Future;
use std::pin::Pin;
use crate::collections::watched::WatchedValue;
use std::collections::VecDeque;
use crate::protocols::tcp::segment::{TcpSegment, TcpSegmentDecoder, TcpSegmentEncoder};
use crate::fail::Fail;
use crate::event::Event;
use std::convert::TryFrom;
use std::collections::HashMap;
use std::num::Wrapping;
use futures_intrusive::channel::LocalChannel;
use crate::runtime::Runtime;
use std::rc::Rc;
use std::cell::{Cell, RefCell};
use std::time::{Instant, Duration};
use super::rto::RtoCalculator;
use futures::FutureExt;
use futures::future::{self, Either};

pub struct Receiver {
    open: Cell<bool>,

    //                     |-----------------recv_window-------------------|
    //                base_seq_no             ack_seq_no             recv_seq_no
    //                     v                       v                       v
    // ... ----------------|-----------------------|-----------------------| (unavailable)
    //         received           acknowledged           unacknowledged
    //
    pub base_seq_no: WatchedValue<SeqNumber>,
    pub recv_queue: RefCell<VecDeque<Vec<u8>>>,
    pub ack_seq_no: WatchedValue<SeqNumber>,
    pub recv_seq_no: WatchedValue<SeqNumber>,

    pub ack_deadline: WatchedValue<Option<Instant>>,

    pub max_window_size: u32,
}

impl Receiver {
    pub fn window_size(&self) -> u32 {
        let Wrapping(bytes_outstanding) = self.recv_seq_no.get() - self.base_seq_no.get();
        self.max_window_size - bytes_outstanding
    }

    pub fn current_ack(&self) -> Option<SeqNumber> {
        let ack_seq_no = self.ack_seq_no.get();
        let recv_seq_no = self.recv_seq_no.get();
        if ack_seq_no < recv_seq_no { Some(recv_seq_no) } else { None }
    }

    pub fn ack_sent(&self, seq_no: SeqNumber) {
        assert_eq!(seq_no, self.recv_seq_no.get());
        self.ack_deadline.set(None);
        self.ack_seq_no.set(seq_no);
    }

    pub fn recv(&self) -> Result<Option<Vec<u8>>, Fail> {
        if !self.open.get() {
            return Err(Fail::ResourceNotFound { details: "Receiver closed" });
        }

        if self.base_seq_no.get() == self.recv_seq_no.get() {
            return Ok(None);
        }

        let segment = self.recv_queue.borrow_mut().pop_front()
            .expect("recv_seq > base_seq without data in queue?");
        self.base_seq_no.modify(|b| b + Wrapping(segment.len() as u32));

        Ok(Some(segment))
    }

    pub fn receive_segment(&self, seq_no: SeqNumber, buf: Vec<u8>, now: Instant) -> Result<(), Fail> {
        if !self.open.get() {
            return Err(Fail::ResourceNotFound { details: "Receiver closed" });
        }

        if self.recv_seq_no.get() != seq_no {
            return Err(Fail::Ignored { details: "Out of order segment" });
        }

        let unread_bytes = self.recv_queue.borrow().iter().map(|b| b.len()).sum::<usize>();
        if unread_bytes + buf.len() > self.max_window_size as usize {
            return Err(Fail::Ignored { details: "Full receive window" });
        }

        self.recv_seq_no.modify(|r| r + Wrapping(buf.len() as u32));
        self.recv_queue.borrow_mut().push_back(buf);

        // TODO: How do we handle when the other side is in PERSIST state here?
        if self.ack_deadline.get().is_none() {
            // TODO: Configure this value (and also maybe just have an RT pointer here.)
            self.ack_deadline.set(Some(now + Duration::from_millis(500)));
        }

        Ok(())
    }
}