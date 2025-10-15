use crate::renderer::{RenderVars, render_template};

pub async fn render_room_view(vars: &RenderVars, max_width: usize) -> String {
    let res = [
        "{c:blue}--------------------------------------------------{c}",
        "{c:blue}{rv:title|%*50s}{c}",
        "{c:blue}--------------------------------------------------{c}",
        "\n",
        "{c:white:bold}{rv:body}{c}",
        "\n",
        "{c:green}Items:{c} {rv:items}",
        "{c:green}Exits:{c} {rv:exits}",
        "\n",
    ];

    render_template(res.join("\n").as_str(), vars, max_width)
}
