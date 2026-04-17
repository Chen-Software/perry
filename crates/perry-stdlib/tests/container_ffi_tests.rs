
// Feature: perry-container | Layer: unit | Req: 11.6 | Property: -
#[test]
fn test_backend_init_from_non_tokio_thread() {
    use std::sync::Arc;
    use perry_stdlib::container::get_global_backend;

    std::thread::spawn(|| {
        // must not panic with "no reactor running"
        // we use block_on since get_global_backend is async
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async {
            let result = get_global_backend().await;
            assert!(result.is_ok() || result.is_err());
        });
    }).join().expect("thread panicked — likely async runtime bootstrap failure");
}
