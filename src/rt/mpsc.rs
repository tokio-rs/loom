use crate::rt::{object, Access, Synchronize, VersionVec};
use std::sync::atomic::Ordering::{Acquire, Release};

#[derive(Debug)]
pub(crate) struct Channel {
    state: object::Ref<State>,
}

#[derive(Debug)]
pub(super) struct State {
    /// Count of messages in the channel.
    msg_cnt: usize,

    /// Last access that was a send operation.
    last_send_access: Option<Access>,
    /// Last access that was a receive operation.
    last_recv_access: Option<Access>,

    /// Causality transfers between threads
    synchronize: Synchronize,
}

/// Actions performed on the Channel.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum Action {
    /// Send a message
    MsgSend,
    /// Receive a message
    MsgRecv,
}

impl Channel {
    pub(crate) fn new() -> Self {
        super::execution(|execution| {
            let state = execution.objects.insert(State {
                msg_cnt: 0,
                last_send_access: None,
                last_recv_access: None,
                synchronize: Synchronize::new(),
            });
            Self { state }
        })
    }

    pub(crate) fn send(&self) {
        self.state.branch_action(Action::MsgSend);
        super::execution(|execution| {
            let state = self.state.get_mut(&mut execution.objects);
            state.msg_cnt = state.msg_cnt.checked_add(1).expect("overflow");

            state
                .synchronize
                .sync_store(&mut execution.threads, Release);

            if state.msg_cnt == 1 {
                // Unblock all threads that are blocked waiting on this channel
                let thread_id = execution.threads.active_id();
                for (id, thread) in execution.threads.iter_mut() {
                    if id == thread_id {
                        continue;
                    }

                    let obj = thread
                        .operation
                        .as_ref()
                        .map(|operation| operation.object());

                    if obj == Some(self.state.erase()) {
                        thread.set_runnable();
                    }
                }
            }
        })
    }

    pub(crate) fn recv(&self) {
        self.state.branch_disable(Action::MsgRecv, self.is_empty());
        super::execution(|execution| {
            let state = self.state.get_mut(&mut execution.objects);
            let thread_id = execution.threads.active_id();
            state.msg_cnt = state
                .msg_cnt
                .checked_sub(1)
                .expect("expected to be able to read the message");
            dbg!(state.synchronize.sync_load(&mut execution.threads, Acquire));
            if state.msg_cnt == 0 {
                // Block all **other** threads attempting to read from the channel
                for (id, thread) in execution.threads.iter_mut() {
                    if id == thread_id {
                        continue;
                    }

                    if let Some(operation) = thread.operation.as_ref() {
                        if operation.object() == self.state.erase()
                            && operation.action() == object::Action::Channel(Action::MsgRecv)
                        {
                            thread.set_blocked();
                        }
                    }
                }
            }
        })
    }

    /// Returns `true` if the channel is currently empty
    fn is_empty(&self) -> bool {
        super::execution(|execution| self.get_state(&mut execution.objects).msg_cnt == 0)
    }

    fn get_state<'a>(&self, objects: &'a mut object::Store) -> &'a mut State {
        self.state.get_mut(objects)
    }
}

impl State {
    pub(super) fn check_for_leaks(&self) {
        assert_eq!(0, self.msg_cnt, "Messages leaked");
    }

    pub(super) fn last_dependent_access(&self, action: Action) -> Option<&Access> {
        match action {
            Action::MsgSend => self.last_send_access.as_ref(),
            Action::MsgRecv => self.last_recv_access.as_ref(),
        }
    }

    pub(super) fn set_last_access(&mut self, action: Action, path_id: usize, version: &VersionVec) {
        match action {
            Action::MsgSend => Access::set_or_create(&mut self.last_send_access, path_id, version),
            Action::MsgRecv => Access::set_or_create(&mut self.last_recv_access, path_id, version),
        }
    }
}
