pub const PLANNER_TEMPLATE: &str = include_str!("planner.txt");
pub const WRITER_TEMPLATE: &str = include_str!("writer.txt");
pub const EDITOR_TEMPLATE: &str = include_str!("editor.txt");
pub const CRITIC_TEMPLATE: &str = include_str!("critic.txt");

pub fn render_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut output = template.to_string();
    for (key, value) in replacements {
        let placeholder = format!("{{{{{key}}}}}");
        output = output.replace(&placeholder, value);
    }
    output
}
