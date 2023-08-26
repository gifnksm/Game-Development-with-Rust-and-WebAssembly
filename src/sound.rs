use anyhow::{anyhow, Result};
use js_sys::ArrayBuffer;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, AudioDestinationNode, AudioNode};

pub(crate) fn create_audio_context() -> Result<AudioContext> {
    AudioContext::new().map_err(|err| anyhow!("could not create audio context: {err:#?}"))
}

fn create_buffer_source(ctx: &AudioContext) -> Result<AudioBufferSourceNode> {
    ctx.create_buffer_source()
        .map_err(|err| anyhow!("could not create buffer source: {err:#?}"))
}

fn connect_with_audio_node(
    buffer_source: &AudioBufferSourceNode,
    destination: &AudioDestinationNode,
) -> Result<AudioNode> {
    buffer_source
        .connect_with_audio_node(destination)
        .map_err(|err| anyhow!("could not connect buffer source with destination: {err:#?}"))
}

fn create_track_source(ctx: &AudioContext, buffer: &AudioBuffer) -> Result<AudioBufferSourceNode> {
    let track_source = create_buffer_source(ctx)?;
    track_source.set_buffer(Some(buffer));
    connect_with_audio_node(&track_source, &ctx.destination())?;
    Ok(track_source)
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Looping {
    No,
    Yes,
}

pub(crate) fn play_sound(ctx: &AudioContext, buffer: &AudioBuffer, looping: Looping) -> Result<()> {
    let track_source = create_track_source(ctx, buffer)?;
    if matches!(looping, Looping::Yes) {
        track_source.set_loop(true);
    }

    track_source
        .start()
        .map_err(|err| anyhow!("could not start track: {err:#?}"))
}

pub(crate) async fn decode_audio_data(
    ctx: &AudioContext,
    array_buffer: &ArrayBuffer,
) -> Result<AudioBuffer> {
    JsFuture::from(
        ctx.decode_audio_data(array_buffer)
            .map_err(|err| anyhow!("could not decode audio data: {err:#?}"))?,
    )
    .await
    .map_err(|err| anyhow!("error decoding audio data: {err:#?}"))?
    .dyn_into()
    .map_err(|err| anyhow!("error converting {err:#?} to `AudioBuffer`"))
}
