use std::{borrow::Cow, ops::Deref};

use crate::{error::Error, inputs::InputInvocation};

use ergo_database::{object_id::*, PostgresPool, RedisPool};
use ergo_notifications::{Notification, NotificationManager, NotifyEvent};
use ergo_queues::{generic_stage::QueueJob, Queue};
use sqlx::Connection;
use uuid::Uuid;

use super::validate_input_payload;

const QUEUE_NAME: &str = "er-input";

#[derive(Clone)]
pub struct InputQueue(Queue);
impl Deref for InputQueue {
    type Target = Queue;

    fn deref(&self) -> &Queue {
        &self.0
    }
}

impl InputQueue {
    pub fn new(redis_pool: RedisPool) -> InputQueue {
        let queue_name = match redis_pool.key_prefix() {
            Some(prefix) => format!("{}-{}", prefix, QUEUE_NAME),
            None => QUEUE_NAME.to_string(),
        };

        InputQueue(Queue::new(redis_pool, queue_name, None, None, None))
    }
}

pub struct EnqueueInputOptions<'a> {
    pub pg: &'a PostgresPool,
    pub notifications: Option<NotificationManager>,
    pub org_id: OrgId,
    pub task_id: TaskId,
    pub task_name: String,
    pub input_id: InputId,
    pub task_trigger_id: TaskTriggerId,
    pub task_trigger_local_id: String,
    pub task_trigger_name: String,
    pub payload_schema: &'a serde_json::Value,
    pub payload: serde_json::Value,
    pub redis_key_prefix: &'a Option<String>,
}

pub async fn enqueue_input(options: EnqueueInputOptions<'_>) -> Result<Uuid, Error> {
    let EnqueueInputOptions {
        pg,
        notifications,
        org_id,
        task_id,
        task_name,
        input_id,
        task_trigger_id,
        task_trigger_local_id,
        task_trigger_name,
        payload_schema,
        payload,
        redis_key_prefix,
    } = options;

    validate_input_payload(&input_id, payload_schema, &payload)?;

    let input_arrival_id = Uuid::new_v4();
    let queue_name = redis_key_prefix
        .as_ref()
        .map(|prefix| Cow::Owned(format!("{}-{}", prefix, QUEUE_NAME)))
        .unwrap_or(Cow::Borrowed(QUEUE_NAME));

    let mut conn = pg.acquire().await?;
    conn.transaction(|tx| {
        let input_id = input_id.clone();
        let task_id = task_id.clone();
        let task_trigger_id = task_trigger_id.clone();
        Box::pin(async move {
            let invocation = InputInvocation {
                task_trigger_id: task_trigger_id.clone(),
                payload: payload.clone(),
                task_id: task_id.clone(),
                input_id,
                inputs_log_id: input_arrival_id,
            };

            let job = QueueJob {
                queue: queue_name.as_ref(),
                payload: &invocation,
                id: None,
                run_at: None,
                timeout: None,
                max_retries: None,
                retry_backoff: None,
            };

            job.enqueue(&mut *tx).await?;

            sqlx::query!(
                r##"INSERT INTO inputs_log
        (inputs_log_id, task_trigger_id, task_id, task_trigger_local_id, status, payload)
        VALUES
        ($1, $2, $3, $4, 'pending', $5)"##,
                &input_arrival_id,
                &task_trigger_id.0,
                &task_id.0,
                &task_trigger_local_id,
                &payload
            )
            .execute(&mut *tx)
            .await?;

            if let Some(notify) = notifications {
                let notification = Notification {
                    task_id,
                    local_id: task_trigger_local_id,
                    local_object_id: Some(task_trigger_id.into_inner()),
                    local_object_name: task_trigger_name,
                    error: None,
                    event: NotifyEvent::InputArrived,
                    task_name,
                    log_id: Some(input_arrival_id),
                    payload: Some(payload),
                };
                notify.notify(&mut *tx, &org_id, notification).await?;
            }

            Ok::<(), Error>(())
        })
    })
    .await?;

    Ok(input_arrival_id)
}
