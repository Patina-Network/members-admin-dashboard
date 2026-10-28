use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
    session_store,
};

const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Process-local sessions with eviction even when clients never return.
#[derive(Clone, Debug)]
pub(super) struct ExpiringMemoryStore {
    records: Arc<Mutex<HashMap<Id, Record>>>,
}

impl Default for ExpiringMemoryStore {
    fn default() -> Self {
        let records = Arc::new(Mutex::new(HashMap::<Id, Record>::new()));
        let weak = Arc::downgrade(&records);
        // The worker must not keep the store alive after the router is dropped.
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;
                let Some(records) = weak.upgrade() else {
                    break;
                };
                let now = OffsetDateTime::now_utc();
                records
                    .lock()
                    .await
                    .retain(|_, record| record.expiry_date > now);
            }
        });
        Self { records }
    }
}

#[async_trait]
impl SessionStore for ExpiringMemoryStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let mut records = self.records.lock().await;
        while records.contains_key(&record.id) {
            record.id = Id::default();
        }
        records.insert(record.id, record.clone());
        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        self.records.lock().await.insert(record.id, record.clone());
        Ok(())
    }

    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        let mut records = self.records.lock().await;
        if records
            .get(id)
            .is_some_and(|record| record.expiry_date <= OffsetDateTime::now_utc())
        {
            records.remove(id);
        }
        Ok(records.get(id).cloned())
    }

    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        self.records.lock().await.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(expiry_date: OffsetDateTime) -> Record {
        Record {
            id: Id::default(),
            data: HashMap::new(),
            expiry_date,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn evicts_abandoned_sessions_without_a_read_and_preserves_active_sessions() {
        let store = ExpiringMemoryStore::default();
        let mut expired = record(OffsetDateTime::now_utc() - time::Duration::seconds(1));
        let mut active = record(OffsetDateTime::now_utc() + time::Duration::hours(1));
        store.create(&mut expired).await.unwrap();
        store.create(&mut active).await.unwrap();
        assert_eq!(store.records.lock().await.len(), 2);

        tokio::task::yield_now().await;
        tokio::time::advance(CLEANUP_INTERVAL).await;
        tokio::task::yield_now().await;

        assert_eq!(store.records.lock().await.len(), 1);
        assert_eq!(store.load(&active.id).await.unwrap(), Some(active));
        let weak = Arc::downgrade(&store.records);
        drop(store);
        assert!(weak.upgrade().is_none());
    }

    #[tokio::test]
    async fn handles_collisions_updates_expiry_and_deletion() {
        let store = ExpiringMemoryStore::default();
        let mut first = record(OffsetDateTime::now_utc() + time::Duration::hours(1));
        store.create(&mut first).await.unwrap();
        let mut second = first.clone();
        store.create(&mut second).await.unwrap();
        assert_ne!(first.id, second.id);

        first.expiry_date = OffsetDateTime::now_utc() - time::Duration::seconds(1);
        store.save(&first).await.unwrap();
        assert!(store.load(&first.id).await.unwrap().is_none());
        assert!(!store.records.lock().await.contains_key(&first.id));
        assert_eq!(store.load(&second.id).await.unwrap(), Some(second.clone()));
        store.delete(&second.id).await.unwrap();
        assert!(store.records.lock().await.is_empty());
    }
}
