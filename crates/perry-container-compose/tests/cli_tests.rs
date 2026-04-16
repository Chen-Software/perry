// Feature: perry-container | Layer: unit | Req: 8.1 | Property: -

#[cfg(test)]
mod tests {
    use clap::Parser;
    use perry_container_compose::cli::{Cli, Commands};
    use std::path::PathBuf;

    #[test]
    fn test_cli_parse_global_options() {
        let args = vec![
            "perry-compose",
            "-f", "compose.yaml",
            "-p", "my-project",
            "--env-file", ".env.test",
            "up",
        ];
        let cli = Cli::parse_from(args);
        assert_eq!(cli.files, vec![PathBuf::from("compose.yaml")]);
        assert_eq!(cli.project_name, Some("my-project".to_string()));
        assert_eq!(cli.env_files, vec![PathBuf::from(".env.test")]);
        assert!(matches!(cli.command, Commands::Up(_)));
    }

    #[test]
    fn test_cli_parse_up_flags() {
        let args = vec!["perry-compose", "up", "-d", "--build", "web", "db"];
        let cli = Cli::parse_from(args);
        if let Commands::Up(up_args) = cli.command {
            assert!(up_args.detach);
            assert!(up_args.build);
            assert_eq!(up_args.services, vec!["web", "db"]);
        } else {
            panic!("Expected Up command");
        }
    }

    #[test]
    fn test_cli_parse_down_volumes() {
        let args = vec!["perry-compose", "down", "-v"];
        let cli = Cli::parse_from(args);
        if let Commands::Down(down_args) = cli.command {
            assert!(down_args.volumes);
        } else {
            panic!("Expected Down command");
        }
    }

    #[test]
    fn test_cli_parse_exec_trailing() {
        let args = vec!["perry-compose", "exec", "web", "ls", "-la", "/tmp"];
        let cli = Cli::parse_from(args);
        if let Commands::Exec(exec_args) = cli.command {
            assert_eq!(exec_args.service, "web");
            assert_eq!(exec_args.cmd, vec!["ls", "-la", "/tmp"]);
        } else {
            panic!("Expected Exec command");
        }
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 8.1         | test_cli_parse_global_options | unit |
| 8.2         | test_cli_parse_up_flags | unit |
| 8.3         | test_cli_parse_up_flags | unit |
| 8.4         | test_cli_parse_down_volumes | unit |
| 8.6         | test_cli_parse_exec_trailing | unit |
*/

// Deferred Requirements: none
