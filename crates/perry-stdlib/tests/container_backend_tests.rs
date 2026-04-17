// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -

#[cfg(test)]
mod tests {
    use perry_stdlib::container::backend::*;
    use std::path::PathBuf;

    // Feature: perry-container | Layer: unit | Req: 1.1 | Property: -
    #[test]
    fn test_backend_reexports_presence() {
        // Compile-time check that re-exports from perry-container-compose are present
        let _driver = BackendDriver::Docker { bin: PathBuf::from("docker") };
        let _builder = OciCommandBuilder;
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 1.1         | test_backend_reexports_presence | unit |
| 11.3        | test_backend_reexports_presence | unit |
*/

// Deferred Requirements: none
