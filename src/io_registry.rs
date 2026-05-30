use crate::ast::Type;

pub struct IoConcept {
    pub concept: &'static str,
    pub symbol: &'static str,
    pub ty: Type,
    pub has_param: bool,
    pub description: &'static str,
}

pub fn io_lookup(concept: &str) -> Option<&'static IoConcept> {
    IO_CONCEPTS.iter().find(|c| c.concept == concept)
}

pub fn list_concepts() -> String {
    IO_CONCEPTS
        .iter()
        .map(|c| format!("  {} -> {} ({})", c.concept, c.symbol, c.description))
        .collect::<Vec<_>>()
        .join("\n")
}

const IO_CONCEPTS: &[IoConcept] = &[
    IoConcept {
        concept: "sigint",
        symbol: "__sigint_flag",
        ty: Type::Bool,
        has_param: false,
        description: "SIGINT interrupt (Ctrl+C)",
    },
    IoConcept {
        concept: "sigterm",
        symbol: "__sigterm_flag",
        ty: Type::Bool,
        has_param: false,
        description: "SIGTERM termination signal",
    },
    IoConcept {
        concept: "sighup",
        symbol: "__sighup_flag",
        ty: Type::Bool,
        has_param: false,
        description: "SIGHUP hangup signal",
    },
    IoConcept {
        concept: "stdin_ready",
        symbol: "__stdin_ready",
        ty: Type::Bool,
        has_param: false,
        description: "Stdin has data available",
    },
    IoConcept {
        concept: "stdin_line",
        symbol: "__stdin_buffer",
        ty: Type::String,
        has_param: false,
        description: "Current stdin line buffer",
    },
    IoConcept {
        concept: "timer(1hz)",
        symbol: "__io_timer_1hz",
        ty: Type::Int,
        has_param: true,
        description: "1-second timer tick",
    },
    IoConcept {
        concept: "timer(100hz)",
        symbol: "__io_timer_100hz",
        ty: Type::Int,
        has_param: true,
        description: "10ms timer tick",
    },
    IoConcept {
        concept: "io_pending",
        symbol: "__io_pending",
        ty: Type::Bool,
        has_param: false,
        description: "Generic IO pending flag",
    },
    IoConcept {
        concept: "mouse_click",
        symbol: "__io_mouse_click",
        ty: Type::Bool,
        has_param: false,
        description: "Mouse button click",
    },
    IoConcept {
        concept: "key_press",
        symbol: "__io_key_press",
        ty: Type::Char,
        has_param: false,
        description: "Keyboard key press",
    },
];
