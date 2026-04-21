pub const PERRY_CONTAINER_TABLE: &[(&str, &str)] = &[
    ("run",         "js_container_run"),
    ("create",      "js_container_create"),
    ("start",       "js_container_start"),
    ("stop",        "js_container_stop"),
    ("remove",      "js_container_remove"),
    ("list",        "js_container_list"),
    ("inspect",     "js_container_inspect"),
    ("logs",        "js_container_logs"),
    ("exec",        "js_container_exec"),
    ("pullImage",   "js_container_pullImage"),
    ("listImages",  "js_container_listImages"),
    ("removeImage", "js_container_removeImage"),
    ("getBackend",  "js_container_getBackend"),
    ("composeUp",   "js_container_composeUp"),
];

pub const PERRY_COMPOSE_TABLE: &[(&str, &str)] = &[
    ("up",      "js_compose_up"),
    ("down",    "js_compose_down"),
    ("ps",      "js_compose_ps"),
    ("logs",    "js_compose_logs"),
    ("exec",    "js_compose_exec"),
    ("config",  "js_compose_config"),
    ("start",   "js_compose_start"),
    ("stop",    "js_compose_stop"),
    ("restart", "js_compose_restart"),
];
