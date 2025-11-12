// Legacy channel implementations - these were based on old trait definitions
// that have been superseded by the distributor module.
// Keeping this file for reference but commented out.

// use crate::api::{SPMCNethunsChannel, SPMCPopper, SPMCPusher};
// 
// impl<const BATCH_SIZE: usize, T: Send + 'static> SPMCNethunsChannel<{ BATCH_SIZE }, T>
//     for (
//         flume::Sender<[T; BATCH_SIZE]>,
//         flume::Receiver<[T; BATCH_SIZE]>,
//     )
// {
//     fn split(
//         self,
//     ) -> (
//         impl SPMCPusher<{ BATCH_SIZE }, T> + 'static,
//         impl SPMCPopper<{ BATCH_SIZE }, T> + 'static,
//     ) {
//         self
//     }
// }
// 
// 
// impl<const BATCH_SIZE: usize, T: 'static + Send> SPMCPusher<{ BATCH_SIZE }, T>
//     for flume::Sender<[T; BATCH_SIZE]>
// {
//     fn push(&self, batch: [T; BATCH_SIZE]) -> core::result::Result<(), [T; BATCH_SIZE]> {
//         self.send(batch).map_err(|error| error.0)
//     }
// }
// 
// impl<const BATCH_SIZE: usize, T: 'static + Send> SPMCPopper<{ BATCH_SIZE }, T>
//     for flume::Receiver<[T; BATCH_SIZE]>
// {
//     fn pop(&self) -> Option<[T; BATCH_SIZE]> {
//         let batch = self.recv().ok()?;
//         Some(batch)
//     }
// }
