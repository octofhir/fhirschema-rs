use async_trait::async_trait;
use octofhir_fhirschema::validation::SchemaCompiler;
use octofhir_fhirschema::{FhirSchema, SchemaProvider};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Provider {
    schemas: HashMap<String, Arc<FhirSchema>>,
    calls: AtomicUsize,
}

#[async_trait]
impl SchemaProvider for Provider {
    async fn get_schema(&self, name: &str) -> Option<Arc<FhirSchema>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        tokio::task::yield_now().await;
        self.schemas.get(name).cloned()
    }
}

fn schema(name: &str, child: Option<&str>) -> Arc<FhirSchema> {
    Arc::new(
        serde_json::from_value(serde_json::json!({
            "name": name, "type": name, "url": format!("http://test/{name}"),
            "kind":"complex-type", "class":"type",
            "elements": child.map(|child| serde_json::json!({"child":{"type":child}})),
        }))
        .unwrap(),
    )
}

#[tokio::test]
async fn concurrent_misses_coalesce_and_invalidation_recompiles() {
    let provider = Arc::new(Provider {
        schemas: HashMap::from([("A".into(), schema("A", None))]),
        calls: AtomicUsize::new(0),
    });
    let compiler = Arc::new(SchemaCompiler::new(provider.clone()));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let compiler = compiler.clone();
        tasks.push(tokio::spawn(
            async move { compiler.compile("A").await.unwrap() },
        ));
    }
    let first = tasks.remove(0).await.unwrap();
    for task in tasks {
        assert!(Arc::ptr_eq(&first, &task.await.unwrap()));
    }
    assert_eq!(provider.calls.load(Ordering::Relaxed), 1);
    compiler.invalidate();
    assert!(!Arc::ptr_eq(&first, &compiler.compile("A").await.unwrap()));
    assert_eq!(provider.calls.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn mutually_recursive_roots_return_errors_without_waiting_on_each_other() {
    let provider = Arc::new(Provider {
        schemas: HashMap::from([
            ("A".into(), schema("A", Some("B"))),
            ("B".into(), schema("B", Some("A"))),
        ]),
        calls: AtomicUsize::new(0),
    });
    let compiler = SchemaCompiler::new(provider);
    let (a, b) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::join!(compiler.compile("A"), compiler.compile("B"))
    })
    .await
    .expect("Cyclic schema compilation deadlocked");
    assert!(a.is_err());
    assert!(b.is_err());
}

#[tokio::test]
async fn inline_overlays_cannot_bypass_the_cycle_guard() {
    let recursive: FhirSchema = serde_json::from_value(serde_json::json!({
        "name":"A", "type":"A", "url":"http://test/A", "kind":"complex-type", "class":"type",
        "elements":{"child":{"type":"A","elements":{"value":{"type":"string"}}}}
    }))
    .unwrap();
    let provider = Arc::new(Provider {
        schemas: HashMap::from([("A".into(), Arc::new(recursive))]),
        calls: AtomicUsize::new(0),
    });
    let compiler = SchemaCompiler::new(provider);
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), compiler.compile("A"))
        .await
        .expect("Inline overlay recursion did not terminate");
    assert!(result.unwrap_err().message.contains("Recursive"));
}

struct PausedProvider {
    schema: std::sync::RwLock<Option<Arc<FhirSchema>>>,
    calls: AtomicUsize,
    started: tokio::sync::Semaphore,
    resume: tokio::sync::Semaphore,
}

#[async_trait]
impl SchemaProvider for PausedProvider {
    async fn get_schema(&self, _name: &str) -> Option<Arc<FhirSchema>> {
        let snapshot = self.schema.read().unwrap().clone();
        if self.calls.fetch_add(1, Ordering::Relaxed) == 0 {
            self.started.add_permits(1);
            self.resume.acquire().await.unwrap().forget();
        }
        snapshot
    }
}

#[tokio::test]
async fn invalidation_discards_in_flight_successes_and_errors() {
    for old in [Some(schema("old", None)), None] {
        let provider = Arc::new(PausedProvider {
            schema: std::sync::RwLock::new(old),
            calls: AtomicUsize::new(0),
            started: tokio::sync::Semaphore::new(0),
            resume: tokio::sync::Semaphore::new(0),
        });
        let compiler = Arc::new(SchemaCompiler::new(provider.clone()));
        let task = tokio::spawn({
            let compiler = compiler.clone();
            async move { compiler.compile("A").await }
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            provider.started.acquire(),
        )
        .await
        .unwrap()
        .unwrap()
        .forget();
        *provider.schema.write().unwrap() = Some(schema("new", None));
        compiler.invalidate();
        provider.resume.add_permits(1);
        let compiled = tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(compiled.name, "new");
        assert!(Arc::ptr_eq(
            &compiled,
            &compiler.compile("A").await.unwrap()
        ));
        assert_eq!(provider.calls.load(Ordering::Relaxed), 2);
    }
}

#[tokio::test]
async fn new_waiter_does_not_accept_an_old_generations_error() {
    let provider = Arc::new(PausedProvider {
        schema: std::sync::RwLock::new(None),
        calls: AtomicUsize::new(0),
        started: tokio::sync::Semaphore::new(0),
        resume: tokio::sync::Semaphore::new(0),
    });
    let compiler = Arc::new(SchemaCompiler::new(provider.clone()));
    let old = tokio::spawn({
        let compiler = compiler.clone();
        async move { compiler.compile("A").await }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        provider.started.acquire(),
    )
    .await
    .unwrap()
    .unwrap()
    .forget();
    *provider.schema.write().unwrap() = Some(schema("new", None));
    compiler.invalidate();
    let new = tokio::spawn({
        let compiler = compiler.clone();
        async move { compiler.compile("A").await }
    });
    tokio::task::yield_now().await;
    provider.resume.add_permits(1);
    for task in [old, new] {
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(result.name, "new");
    }
}
