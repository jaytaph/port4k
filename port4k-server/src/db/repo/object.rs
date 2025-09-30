use crate::domain::{RenderObject, RoomObject};

pub struct ObjectRepo;

impl ObjectRepo {
    pub async fn list_for_room(
        &self,
        c: &tokio_postgres::Client,
        bp: &str,
        room: &str,
    ) -> anyhow::Result<Vec<RoomObject>> {
        let rows = c.query(
            r#"
            SELECT o.id, o.short, o.description, o.examine, o.state, o.use_lua, o.position,
                   COALESCE((SELECT array_agg(noun ORDER BY noun)
                              FROM bp_object_nouns n
                             WHERE n.bp_key=o.bp_key AND n.room_key=o.room_key AND n.obj_id=o.id), ARRAY[]::text[])
              FROM bp_objects o
             WHERE o.bp_key=$1 AND o.room_key=$2
             ORDER BY o.position NULLS LAST, o.id
            "#,
            &[&bp, &room],
        ).await?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(RoomObject {
                id: r.get(0),
                short: r.get(1),
                description: r.get(2),
                examine: r.get(3),
                state: r.get(4),
                use_lua: r.get(5),
                position: r.get(6),
                nouns: r.get::<_, Vec<String>>(7),
            });
        }
        Ok(out)
    }

    pub async fn render_projection(
        &self,
        c: &tokio_postgres::Client,
        bp: &str,
        room: &str,
    ) -> anyhow::Result<Vec<RenderObject>> {
        let rows = c
            .query(
                "SELECT id, short FROM bp_objects WHERE bp_key=$1 AND room_key=$2",
                &[&bp, &room],
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| RenderObject {
                id: r.get(0),
                short: r.get(1),
            })
            .collect())
    }
}
