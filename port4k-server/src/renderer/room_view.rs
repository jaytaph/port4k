use crate::renderer::{render_template, RenderVars};

pub async fn render_room_view(vars: &RenderVars, max_width: usize) -> String {
    let mut res = Vec::new();
    res.push("{c:blue}--------------------------------------------------{c}");
    res.push("{c:blue}{rv:title|%*50s}{c}");
    res.push("{c:blue}--------------------------------------------------{c}");
    res.push("\n");
    res.push("{c:white:bold}{rv:body}{c}");
    res.push("\n");
    res.push("{c:green}Items:{c} {rv:items}");
    res.push("{c:green}Exits:{c} {rv:exits}");
    res.push("\n");

    render_template(res.join("\n").as_str(), vars, max_width)
}