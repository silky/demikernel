// -*- mode: c++; c-file-style: "k&r"; c-basic-offset: 4 -*-
/***********************************************************************
 *
 * common/queue.h
 *   Basic queue
 *
 * Copyright 2018 Irene Zhang  <irene.zhang@microsoft.com>
 *
 * Permission is hereby granted, free of charge, to any person
 * obtaining a copy of this software and associated documentation
 * files (the "Software"), to deal in the Software without
 * restriction, including without limitation the rights to use, copy,
 * modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
 * BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
 * ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 *
 **********************************************************************/

#ifndef DMTR_LIBOS_BASIC_QUEUE_HH_IS_INCLUDED
#define DMTR_LIBOS_BASIC_QUEUE_HH_IS_INCLUDED

#include "io_queue.hh"

#include <condition_variable>
#include <dmtr/types.h>
#include <memory>
#include <mutex>
#include <queue>
#include <unordered_map>

namespace dmtr {

class memory_queue : public io_queue
{
    private: std::queue<dmtr_sgarray_t> my_ready_queue;
    private: std::recursive_mutex my_lock;
    private: std::unique_ptr<task::thread_type> my_push_thread;
    private: std::unique_ptr<task::thread_type> my_pop_thread;
    private: bool my_good_flag;

    private: memory_queue(int qd);
    public: static int new_object(std::unique_ptr<io_queue> &q_out, int qd);

    public: virtual int push(dmtr_qtoken_t qt, const dmtr_sgarray_t &sga);
    public: virtual int pop(dmtr_qtoken_t qt);
    public: virtual int poll(dmtr_qresult_t &qr_out, dmtr_qtoken_t qt);
    public: virtual int drop(dmtr_qtoken_t qt);
    public: virtual int close();

    private: bool good() const {
        return my_good_flag;
    }

    private: void start_threads();
    private: int push_thread(task::thread_type::yield_type &yield, task::thread_type::queue_type &tq);
    private: int pop_thread(task::thread_type::yield_type &yield, task::thread_type::queue_type &tq);
};

} // namespace dmtr

#endif /* DMTR_LIBOS_BASIC_QUEUE_HH_IS_INCLUDED */