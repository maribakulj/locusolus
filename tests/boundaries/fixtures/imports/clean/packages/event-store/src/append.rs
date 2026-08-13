use sqlx::PgPool;

use crate::envelope::Envelope;

pub async fn append(pool: &PgPool, envelope: Envelope) -> Result<(), sqlx::Error> {
    let _ = (pool, envelope);
    Ok(())
}
