use crate::renderer::{render_template, RenderVars};

pub async fn render_room_view(vars: &RenderVars, max_width: usize) -> String {
    let mut res = Vec::new();

    res.push("{c:blue}--------------------------------------------------{c}");
    res.push("{c:blue}{rv:title|20s}{c}");
    res.push("{c:blue}--------------------------------------------------{c}");
    res.push("\n");
    res.push("{c:white:bold}{rv:body}{c}");
    res.push("\n");
    res.push("{c:green}Exits:{c} {rv:exits}");
    res.push("\n");

    render_template(res.join("\n").as_str(), vars, max_width);

    // let dirs: Vec<String> = rows.into_iter().map(|row| row.get(0)).collect();
    // let exits_line = if dirs.is_empty() {
    //     "Exits: none".to_string()
    // } else {
    //     format!("Exits: {}", dirs.join(", "))
    // };
    //
    // Ok(format!("{title}\n{body}\n{exits_line}\n"))

    res.join("\n")
}

// #[allow(unused)]
// pub async fn room_coin_total(&self, room_id: i64) -> DbResult<i64> {
//     let client = self.pool.get().await?;
//     let row = client
//         .query_one(
//             "SELECT COALESCE(SUM(qty), 0)
//                  FROM room_loot
//                  WHERE room_id = $1 AND item = 'coin' AND picked_by IS NULL",
//             &[&room_id],
//         )
//         .await?;
//     Ok(row.get(0))
// }
