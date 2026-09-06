use super::{Database, DbResult};
use base64::{Engine, engine::general_purpose::STANDARD};
use r2d2_sqlite::rusqlite;
use r2d2_sqlite::rusqlite::types::Value as SqlValue;
use r2d2_sqlite::rusqlite::types::ValueRef;
use serde_json::{Value, json};

impl Database {
    pub fn raw_query(&self, sql: &str, params: &[SqlValue], limit: usize) -> DbResult<Vec<Value>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(sql)?;

        let columns: Vec<String> = statement
            .column_names()
            .iter()
            .map(ToString::to_string)
            .collect();

        let mut rows = statement.query(rusqlite::params_from_iter(params.iter()))?;
        let mut output = Vec::new();

        while output.len() < limit {
            let Some(row) = rows.next()? else {
                break;
            };

            let mut object = serde_json::Map::new();

            for (index, name) in columns.iter().enumerate() {
                let value = row.get_ref(index)?;
                object.insert(name.clone(), sqlite_value_to_json(value));
            }

            output.push(Value::Object(object));
        }

        Ok(output)
    }
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::String(value.to_string()),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => json!({ "blob_base64": STANDARD.encode(value) }),
    }
}
