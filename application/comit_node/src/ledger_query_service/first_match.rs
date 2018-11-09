use futures::{stream::Stream, Future};
use ledger_query_service::{
    fetch_transaction_stream::FetchTransactionStream, FetchFullQueryResults, QueryId,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use swap_protocols::ledger::Ledger;
use tokio::timer::Interval;

#[derive(Debug, Clone)]
pub struct FirstMatch<L: Ledger> {
    pub fetch_results: Arc<FetchFullQueryResults<L>>,
    pub poll_interval: Duration,
}

impl<L: Ledger> FirstMatch<L> {
    pub fn first_match_of<E>(
        &self,
        query_id: QueryId<L>,
    ) -> impl Future<Item = L::Transaction, Error = E> {
        self.fetch_results
            .fetch_transaction_stream(Interval::new(Instant::now(), self.poll_interval), query_id)
            .take(1)
            .into_future()
            .map(|(txid, _)| txid.expect("ticker stream should never terminate"))
            .map_err(|(e, _)| panic!("timer should never error but {:?}", e))
    }
}
