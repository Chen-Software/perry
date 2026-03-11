//! HIR WidgetNode → SwiftUI source emitter
//!
//! Walks the declarative widget tree and emits valid SwiftUI source code.

use perry_hir::ir::*;
use std::fmt::Write;

/// Emit the TimelineEntry struct
pub fn emit_entry_struct(widget: &WidgetDecl, name: &str) -> String {
    let mut out = String::new();
    writeln!(out, "struct {}Entry: TimelineEntry {{", name).unwrap();
    writeln!(out, "    let date: Date").unwrap();
    for (field_name, field_type) in &widget.entry_fields {
        let swift_type = match field_type {
            WidgetFieldType::String => "String",
            WidgetFieldType::Number => "Double",
            WidgetFieldType::Boolean => "Bool",
        };
        writeln!(out, "    let {}: {}", field_name, swift_type).unwrap();
    }
    writeln!(out, "}}").unwrap();
    out
}

/// Emit the SwiftUI View from the render body
pub fn emit_view(widget: &WidgetDecl, name: &str) -> String {
    let mut out = String::new();
    let entry_param = &widget.entry_param_name;

    writeln!(out, "struct {}View: View {{", name).unwrap();
    writeln!(out, "    let {}: {}Entry", entry_param, name).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    var body: some View {{").unwrap();

    // Emit the render tree
    if widget.render_body.is_empty() {
        writeln!(out, "        Text(\"Empty widget\")").unwrap();
    } else if widget.render_body.len() == 1 {
        let node_str = emit_node(&widget.render_body[0], entry_param, 2);
        out.push_str(&node_str);
    } else {
        // Multiple root nodes — wrap in VStack
        writeln!(out, "        VStack {{").unwrap();
        for node in &widget.render_body {
            let node_str = emit_node(node, entry_param, 3);
            out.push_str(&node_str);
        }
        writeln!(out, "        }}").unwrap();
    }

    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    out
}

/// Emit the TimelineProvider
pub fn emit_timeline_provider(widget: &WidgetDecl, name: &str) -> String {
    let mut out = String::new();

    writeln!(out, "struct {}Provider: TimelineProvider {{", name).unwrap();
    writeln!(out, "    func placeholder(in context: Context) -> {}Entry {{", name).unwrap();
    // Emit placeholder with default values
    write!(out, "        {}Entry(date: Date()", name).unwrap();
    for (field_name, field_type) in &widget.entry_fields {
        match field_type {
            WidgetFieldType::String => write!(out, ", {}: \"...\"", field_name).unwrap(),
            WidgetFieldType::Number => write!(out, ", {}: 0", field_name).unwrap(),
            WidgetFieldType::Boolean => write!(out, ", {}: false", field_name).unwrap(),
        }
    }
    writeln!(out, ")").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "    func getSnapshot(in context: Context, completion: @escaping ({}Entry) -> ()) {{", name).unwrap();
    writeln!(out, "        completion(placeholder(in: context))").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "    func getTimeline(in context: Context, completion: @escaping (Timeline<{}Entry>) -> ()) {{", name).unwrap();
    writeln!(out, "        let entry = placeholder(in: context)").unwrap();
    writeln!(out, "        let timeline = Timeline(entries: [entry], policy: .atEnd)").unwrap();
    writeln!(out, "        completion(timeline)").unwrap();
    writeln!(out, "    }}").unwrap();

    writeln!(out, "}}").unwrap();
    out
}

/// Emit the @main WidgetBundle
pub fn emit_widget_bundle(widget: &WidgetDecl, name: &str) -> String {
    let mut out = String::new();
    writeln!(out, "@main").unwrap();
    writeln!(out, "struct {}WidgetBundle: SwiftUI.WidgetBundle {{", name).unwrap();
    writeln!(out, "    var body: some Widget {{").unwrap();
    writeln!(out, "        {}Widget()", name).unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "struct {}Widget: Widget {{", name).unwrap();
    writeln!(out, "    let kind: String = \"{}\"", widget.kind).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    var body: some WidgetConfiguration {{").unwrap();
    writeln!(out, "        StaticConfiguration(kind: kind, provider: {}Provider()) {{ entry in", name).unwrap();
    writeln!(out, "            {}View({}: entry)", name, widget.entry_param_name).unwrap();
    writeln!(out, "        }}").unwrap();

    // Display name
    if !widget.display_name.is_empty() {
        writeln!(out, "        .configurationDisplayName(\"{}\")", escape_swift_string(&widget.display_name)).unwrap();
    }
    // Description
    if !widget.description.is_empty() {
        writeln!(out, "        .description(\"{}\")", escape_swift_string(&widget.description)).unwrap();
    }
    // Supported families
    if !widget.supported_families.is_empty() {
        let families: Vec<String> = widget.supported_families.iter()
            .map(|f| format!(".{}", f))
            .collect();
        writeln!(out, "        .supportedFamilies([{}])", families.join(", ")).unwrap();
    }

    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    out
}

/// Emit a single WidgetNode as SwiftUI source
fn emit_node(node: &WidgetNode, entry_param: &str, indent: usize) -> String {
    let mut out = String::new();
    let pad = "    ".repeat(indent);

    match node {
        WidgetNode::Text { content, modifiers } => {
            let text_arg = emit_text_content(content, entry_param);
            write!(out, "{}Text({})", pad, text_arg).unwrap();
            emit_modifiers(&mut out, modifiers, indent);
            writeln!(out).unwrap();
        }
        WidgetNode::Stack { kind, spacing, children, modifiers } => {
            let stack_name = match kind {
                WidgetStackKind::VStack => "VStack",
                WidgetStackKind::HStack => "HStack",
                WidgetStackKind::ZStack => "ZStack",
            };
            if let Some(sp) = spacing {
                write!(out, "{}{}(spacing: {})", pad, stack_name, format_f64(*sp)).unwrap();
            } else {
                write!(out, "{}{}", pad, stack_name).unwrap();
            }
            writeln!(out, " {{").unwrap();
            for child in children {
                out.push_str(&emit_node(child, entry_param, indent + 1));
            }
            write!(out, "{}}}", pad).unwrap();
            emit_modifiers(&mut out, modifiers, indent);
            writeln!(out).unwrap();
        }
        WidgetNode::Image { system_name, modifiers } => {
            write!(out, "{}Image(systemName: \"{}\")", pad, escape_swift_string(system_name)).unwrap();
            emit_modifiers(&mut out, modifiers, indent);
            writeln!(out).unwrap();
        }
        WidgetNode::Spacer => {
            writeln!(out, "{}Spacer()", pad).unwrap();
        }
        WidgetNode::Conditional { field, op, value, then_node, else_node } => {
            let cond = emit_condition(field, op, value, entry_param);
            writeln!(out, "{}if {} {{", pad, cond).unwrap();
            out.push_str(&emit_node(then_node, entry_param, indent + 1));
            if let Some(else_n) = else_node {
                writeln!(out, "{}}} else {{", pad).unwrap();
                out.push_str(&emit_node(else_n, entry_param, indent + 1));
            }
            writeln!(out, "{}}}", pad).unwrap();
        }
    }

    out
}

/// Emit text content as a Swift expression
fn emit_text_content(content: &WidgetTextContent, entry_param: &str) -> String {
    match content {
        WidgetTextContent::Literal(s) => {
            format!("\"{}\"", escape_swift_string(s))
        }
        WidgetTextContent::Field(field) => {
            // Check if field is numeric — if so, use String interpolation
            format!("\"\\({}.{})\"", entry_param, field)
        }
        WidgetTextContent::Template(parts) => {
            let mut s = String::from("\"");
            for part in parts {
                match part {
                    WidgetTemplatePart::Literal(lit) => {
                        s.push_str(&escape_swift_string(lit));
                    }
                    WidgetTemplatePart::Field(field) => {
                        write!(s, "\\({}.{})", entry_param, field).unwrap();
                    }
                }
            }
            s.push('"');
            s
        }
    }
}

/// Emit a condition expression
fn emit_condition(field: &str, op: &WidgetConditionOp, value: &WidgetTextContent, entry_param: &str) -> String {
    let lhs = format!("{}.{}", entry_param, field);
    match op {
        WidgetConditionOp::Truthy => lhs,
        WidgetConditionOp::GreaterThan => {
            format!("{} > {}", lhs, emit_condition_value(value))
        }
        WidgetConditionOp::LessThan => {
            format!("{} < {}", lhs, emit_condition_value(value))
        }
        WidgetConditionOp::Equals => {
            format!("{} == {}", lhs, emit_condition_value(value))
        }
        WidgetConditionOp::NotEquals => {
            format!("{} != {}", lhs, emit_condition_value(value))
        }
    }
}

fn emit_condition_value(value: &WidgetTextContent) -> String {
    match value {
        WidgetTextContent::Literal(s) => {
            // Try as number first
            if let Ok(n) = s.parse::<f64>() {
                format_f64(n)
            } else {
                format!("\"{}\"", escape_swift_string(s))
            }
        }
        WidgetTextContent::Field(f) => f.clone(),
        WidgetTextContent::Template(_) => "\"\"".to_string(),
    }
}

/// Emit SwiftUI modifiers as chained method calls
fn emit_modifiers(out: &mut String, modifiers: &[WidgetModifier], _indent: usize) {
    for modifier in modifiers {
        match modifier {
            WidgetModifier::Font(font) => {
                let font_str = match font {
                    WidgetFont::System(size) => format!(".system(size: {})", format_f64(*size)),
                    WidgetFont::Named(name) => format!(".custom(\"{}\", size: 17)", escape_swift_string(name)),
                    WidgetFont::Headline => ".headline".to_string(),
                    WidgetFont::Title => ".title".to_string(),
                    WidgetFont::Title2 => ".title2".to_string(),
                    WidgetFont::Title3 => ".title3".to_string(),
                    WidgetFont::Body => ".body".to_string(),
                    WidgetFont::Caption => ".caption".to_string(),
                    WidgetFont::Caption2 => ".caption2".to_string(),
                    WidgetFont::Footnote => ".footnote".to_string(),
                    WidgetFont::Subheadline => ".subheadline".to_string(),
                    WidgetFont::LargeTitle => ".largeTitle".to_string(),
                };
                write!(out, "\n{}    .font({})", "    ".repeat(_indent), font_str).unwrap();
            }
            WidgetModifier::FontWeight(weight) => {
                write!(out, "\n{}    .fontWeight(.{})", "    ".repeat(_indent), weight).unwrap();
            }
            WidgetModifier::ForegroundColor(color) => {
                let swift_color = swift_color_expr(color);
                write!(out, "\n{}    .foregroundColor({})", "    ".repeat(_indent), swift_color).unwrap();
            }
            WidgetModifier::Padding(p) => {
                write!(out, "\n{}    .padding({})", "    ".repeat(_indent), format_f64(*p)).unwrap();
            }
            WidgetModifier::Frame { width, height } => {
                let mut args = Vec::new();
                if let Some(w) = width {
                    args.push(format!("width: {}", format_f64(*w)));
                }
                if let Some(h) = height {
                    args.push(format!("height: {}", format_f64(*h)));
                }
                if !args.is_empty() {
                    write!(out, "\n{}    .frame({})", "    ".repeat(_indent), args.join(", ")).unwrap();
                }
            }
            WidgetModifier::CornerRadius(r) => {
                write!(out, "\n{}    .cornerRadius({})", "    ".repeat(_indent), format_f64(*r)).unwrap();
            }
            WidgetModifier::Background(color) => {
                let swift_color = swift_color_expr(color);
                write!(out, "\n{}    .background({})", "    ".repeat(_indent), swift_color).unwrap();
            }
            WidgetModifier::Opacity(o) => {
                write!(out, "\n{}    .opacity({})", "    ".repeat(_indent), format_f64(*o)).unwrap();
            }
            WidgetModifier::LineLimit(n) => {
                write!(out, "\n{}    .lineLimit({})", "    ".repeat(_indent), n).unwrap();
            }
            WidgetModifier::Multiline => {
                write!(out, "\n{}    .lineLimit(nil)", "    ".repeat(_indent)).unwrap();
            }
        }
    }
}

/// Convert a color name to a Swift Color expression
fn swift_color_expr(color: &str) -> String {
    match color {
        "red" => "Color.red".to_string(),
        "blue" => "Color.blue".to_string(),
        "green" => "Color.green".to_string(),
        "white" => "Color.white".to_string(),
        "black" => "Color.black".to_string(),
        "gray" | "grey" => "Color.gray".to_string(),
        "orange" => "Color.orange".to_string(),
        "yellow" => "Color.yellow".to_string(),
        "purple" => "Color.purple".to_string(),
        "pink" => "Color.pink".to_string(),
        "primary" => "Color.primary".to_string(),
        "secondary" => "Color.secondary".to_string(),
        "clear" => "Color.clear".to_string(),
        _ => {
            // Try hex color
            if color.starts_with('#') && color.len() == 7 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&color[1..3], 16),
                    u8::from_str_radix(&color[3..5], 16),
                    u8::from_str_radix(&color[5..7], 16),
                ) {
                    return format!(
                        "Color(red: {:.3}, green: {:.3}, blue: {:.3})",
                        r as f64 / 255.0,
                        g as f64 / 255.0,
                        b as f64 / 255.0
                    );
                }
            }
            format!("Color.{}", color)
        }
    }
}

/// Format f64 without trailing zeros
fn format_f64(v: f64) -> String {
    if v == v.floor() {
        format!("{:.0}", v)
    } else {
        format!("{}", v)
    }
}

/// Escape a string for Swift string literals
fn escape_swift_string(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"', "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_text_widget() {
        let widget = WidgetDecl {
            kind: "com.example.Hello".to_string(),
            display_name: "Hello Widget".to_string(),
            description: "A simple hello widget".to_string(),
            supported_families: vec!["systemSmall".to_string()],
            entry_fields: vec![
                ("greeting".to_string(), WidgetFieldType::String),
            ],
            render_body: vec![
                WidgetNode::Text {
                    content: WidgetTextContent::Field("greeting".to_string()),
                    modifiers: vec![
                        WidgetModifier::Font(WidgetFont::Title),
                        WidgetModifier::ForegroundColor("blue".to_string()),
                    ],
                },
            ],
            entry_param_name: "entry".to_string(),
        };

        let view = emit_view(&widget, "Hello");
        assert!(view.contains("struct HelloView: View"));
        assert!(view.contains("let entry: HelloEntry"));
        assert!(view.contains("Text(\"\\(entry.greeting)\")"));
        assert!(view.contains(".font(.title)"));
        assert!(view.contains(".foregroundColor(Color.blue)"));
    }

    #[test]
    fn test_vstack_with_children() {
        let widget = WidgetDecl {
            kind: "com.test.Stack".to_string(),
            display_name: "Stack".to_string(),
            description: "".to_string(),
            supported_families: vec![],
            entry_fields: vec![
                ("title".to_string(), WidgetFieldType::String),
                ("count".to_string(), WidgetFieldType::Number),
            ],
            render_body: vec![
                WidgetNode::Stack {
                    kind: WidgetStackKind::VStack,
                    spacing: Some(8.0),
                    children: vec![
                        WidgetNode::Text {
                            content: WidgetTextContent::Field("title".to_string()),
                            modifiers: vec![WidgetModifier::Font(WidgetFont::Headline)],
                        },
                        WidgetNode::Text {
                            content: WidgetTextContent::Template(vec![
                                WidgetTemplatePart::Literal("Count: ".to_string()),
                                WidgetTemplatePart::Field("count".to_string()),
                            ]),
                            modifiers: vec![],
                        },
                    ],
                    modifiers: vec![WidgetModifier::Padding(16.0)],
                },
            ],
            entry_param_name: "entry".to_string(),
        };

        let view = emit_view(&widget, "Stack");
        assert!(view.contains("VStack(spacing: 8)"));
        assert!(view.contains(".padding(16)"));
        assert!(view.contains("Text(\"Count: \\(entry.count)\")"));
    }

    #[test]
    fn test_entry_struct() {
        let widget = WidgetDecl {
            kind: "com.test.Entry".to_string(),
            display_name: "".to_string(),
            description: "".to_string(),
            supported_families: vec![],
            entry_fields: vec![
                ("name".to_string(), WidgetFieldType::String),
                ("score".to_string(), WidgetFieldType::Number),
                ("active".to_string(), WidgetFieldType::Boolean),
            ],
            render_body: vec![],
            entry_param_name: "entry".to_string(),
        };

        let s = emit_entry_struct(&widget, "Entry");
        assert!(s.contains("struct EntryEntry: TimelineEntry"));
        assert!(s.contains("let name: String"));
        assert!(s.contains("let score: Double"));
        assert!(s.contains("let active: Bool"));
        assert!(s.contains("let date: Date"));
    }

    #[test]
    fn test_conditional() {
        let widget = WidgetDecl {
            kind: "com.test.Cond".to_string(),
            display_name: "".to_string(),
            description: "".to_string(),
            supported_families: vec![],
            entry_fields: vec![
                ("count".to_string(), WidgetFieldType::Number),
            ],
            render_body: vec![
                WidgetNode::Conditional {
                    field: "count".to_string(),
                    op: WidgetConditionOp::GreaterThan,
                    value: WidgetTextContent::Literal("0".to_string()),
                    then_node: Box::new(WidgetNode::Text {
                        content: WidgetTextContent::Literal("Has items".to_string()),
                        modifiers: vec![],
                    }),
                    else_node: Some(Box::new(WidgetNode::Text {
                        content: WidgetTextContent::Literal("Empty".to_string()),
                        modifiers: vec![],
                    })),
                },
            ],
            entry_param_name: "entry".to_string(),
        };

        let view = emit_view(&widget, "Cond");
        assert!(view.contains("if entry.count > 0"));
        assert!(view.contains("Text(\"Has items\")"));
        assert!(view.contains("} else {"));
        assert!(view.contains("Text(\"Empty\")"));
    }
}
