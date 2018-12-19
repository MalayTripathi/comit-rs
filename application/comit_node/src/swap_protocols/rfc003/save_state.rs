use crate::swap_protocols::rfc003::{state_machine::SwapStates, Role};
use futures::sync::mpsc;
use std::sync::RwLock;

pub trait SaveState<R: Role>: Send + Sync {
    fn save(&self, state: SwapStates<R>);
}

impl<R: Role + Sync> SaveState<R> for RwLock<Option<SwapStates<R>>> {
    fn save(&self, state: SwapStates<R>) {
        let _self = &mut *self.write().unwrap();
        *_self = Some(state);
    }
}

impl<R: Role> SaveState<R> for mpsc::UnboundedSender<SwapStates<R>> {
    fn save(&self, state: SwapStates<R>) {
        // ignore error the subscriber is no longer interested in state updates
        let _ = self.unbounded_send(state);
    }
}
