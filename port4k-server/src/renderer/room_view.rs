use crate::renderer::{RenderVars, render_template};

pub async fn render_room_view(vars: &RenderVars, max_width: usize) -> String {
    let res = [
        "{c:blue}--------------------------------------------------{c}",
        "{c:bright_blue}{rv:title|%*50s}{c}",
        "{c:blue}--------------------------------------------------{c}",
        "\n",
        "{c:bright_white}{rv:body}{c}",
        "\n",
        "Visible items: {c:green}{rv:items}{c}",
        "Visible exits: {c:green}{rv:exits}{c}",
        "\n",
    ];

    render_template(res.join("\n").as_str(), vars, max_width)
}
