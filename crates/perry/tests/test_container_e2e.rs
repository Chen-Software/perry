mod e2e;
use e2e::harness::*;

#[test]
fn test_container_basic() {
    let bin = compile_e2e("crates/perry/tests/e2e/container-basic.e2e.ts").unwrap();
    let result = run_e2e(&bin);
    assert_e2e_pass(&result);
}

#[test]
fn test_workloads_graph() {
    let bin = compile_e2e("crates/perry/tests/e2e/workloads-graph.e2e.ts").unwrap();
    let result = run_e2e(&bin);
    assert_e2e_pass(&result);
}
