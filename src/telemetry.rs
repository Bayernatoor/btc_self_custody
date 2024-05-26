#[cfg(feature = "ssr")]
use {
    tracing::subscriber::set_global_default,
    tracing::Subscriber,
    tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer},
    tracing_log::LogTracer,
    tracing_subscriber::fmt::MakeWriter,
    tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry},
};

#[cfg(feature = "ssr")]
pub fn get_subscriber(
    name: String,
    env_filter: String,
    // A function that returns a sink - a place we can write logs to
    sink: impl for<'a> MakeWriter<'a> + 'static + Send + Sync,
) -> impl Subscriber + Sync + Send {
    // We are falling back to printing all logs at info level or above
    // if the RUST_LOG environment variable is not set.

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name, sink);
    // the with method is provided by `SubscribeExt`, an extension
    // trait for `Subscribe` exposed by `tracing_subcriber`.
    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

/// Register a subscriber as global default to process span data
///
/// Should only be called once!
#[cfg(feature = "ssr")]
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    // redirect all `log`'s events to our subscriber.
    LogTracer::init().expect("Failed to set logger");
    // `set_global_default` can be used by applications to specify
    // what subscriber should be used to process spans.
    set_global_default(subscriber).expect("Failed to set subscriber")
}
