use super::{backend, json, ok, op, opt, opt_str, opt_vec, DeserializeOwned, Serialize, Value};

type StartLab<T> = fn(&backend::BackendEngine, u16, Vec<T>, Option<&str>) -> Result<u16, String>;

pub(super) struct LocalLab<T, R> {
    pub(super) input_key: &'static str,
    pub(super) start: StartLab<T>,
    pub(super) stop: fn(&backend::BackendEngine),
    pub(super) report: fn(&backend::BackendEngine, &[T]) -> R,
}

impl<T, R> LocalLab<T, R> {
    pub(super) fn new(
        input_key: &'static str,
        start: StartLab<T>,
        stop: fn(&backend::BackendEngine),
        report: fn(&backend::BackendEngine, &[T]) -> R,
    ) -> Self {
        Self {
            input_key,
            start,
            stop,
            report,
        }
    }
}

pub(super) fn local_lab<T: DeserializeOwned, R: Serialize>(
    engine: &backend::BackendEngine,
    payload: &Value,
    lab: LocalLab<T, R>,
) -> Result<Value, String> {
    match op(payload) {
        "start" => match (lab.start)(
            engine,
            opt(payload, "port").unwrap_or(0),
            opt_vec(payload, lab.input_key).unwrap_or_default(),
            opt_str(payload, "advertise_url").as_deref(),
        ) {
            Ok(port) => ok(json!({"port":port})),
            Err(error) => Err(error),
        },
        "stop" => {
            (lab.stop)(engine);
            ok(json!({"stopped":true}))
        }
        _ => ok((lab.report)(
            engine,
            &opt_vec(payload, lab.input_key).unwrap_or_default(),
        )),
    }
}
