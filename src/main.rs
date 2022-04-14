use rsmpeg::{
    self,
    avcodec::{AVCodec, AVCodecContext},
    avformat::{
        AVFormatContextInput, AVFormatContextOutput, AVIOContextContainer, AVIOContextCustom,
    },
    avutil::{av_inv_q, av_mul_q, av_rescale_q, av_rescale_q_rnd, AVFrame, AVMem, AVRational},
    error::RsmpegError,
    ffi::{self, AVCodecID_AV_CODEC_ID_H264},
    UnsafeDerefMut,
};
use std::{
    borrow::BorrowMut,
    ffi::CStr,
    fs::File,
    io::{Seek, SeekFrom, Write},
    ops::Deref,
    ptr,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, bail, Context, Result};

use cstr::cstr;

// fn dump_av_info(path: &CStr) -> Result<(), Box<dyn Error>> {
//     let mut input_format_context = AVFormatContextInput::open(path)?;
//     input_format_context.dump(0, path)?;
//     Ok(())
// }

fn main() {
    // dump_av_info(&CString::new("./XingsongSun.jpg").unwrap()).unwrap();

    // std::fs::create_dir_all("output/avio_muxing/").unwrap();

    transcoding(
        cstr!("./1649399982562-123131231-3002-calling.h264"),
        cstr!("./xxxx.mp4"),
    )
    .unwrap();
}

/// Get `video_stream_index`, `input_format_context`, `decode_context`.
fn open_input_file(filename: &CStr) -> Result<(AVFormatContextInput)> {
    let mut input_format_context = AVFormatContextInput::open(filename)?;
    input_format_context.dump(0, filename)?;

    Ok((input_format_context))
}

/// Return output_format_context and encode_context
fn open_output_file(
    filename: &CStr,
    input_format_context: &AVFormatContextInput,
) -> Result<(AVFormatContextOutput)> {
    let mut output_format_context = AVFormatContextOutput::create(filename, None)?;

    let number_of_streams = input_format_context.nb_streams as usize;

    let stream_index = 0;

    let mut streams_list: Vec<i32> = vec![Default::default(); number_of_streams];

    for i in 0..number_of_streams {
        let in_stream = input_format_context.streams().get(0).unwrap();

        streams_list[i] += stream_index;
        let mut out_stream = output_format_context.new_stream();
        let dd = in_stream.codecpar();

        let cc = dd.deref();

        out_stream.set_codecpar(cc.clone())
    }

    output_format_context.dump(0, filename)?;
    output_format_context.write_header()?;

    Ok((output_format_context))
}

/// Transcoding audio and video stream in a multi media file.
pub fn transcoding(input_file: &CStr, output_file: &CStr) -> Result<()> {
    // let mut input_format_context = open_input_file(input_file)?;
    let mut input_format_context = AVFormatContextInput::open(input_file)?;

    // input_format_context.dump(0, input_file)?;

    // let mut output_format_context = open_output_file(output_file, &input_format_context)?;

    let mut output_format_context = AVFormatContextOutput::create(output_file, None)?;

    {
        let mut out_stream = output_format_context.new_stream();

        let (video_index, decoder) = input_format_context
            .find_best_stream(ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)
            .context("Failed to select a video stream")?
            .context("No video stream")?;

        println!("video_index {}", video_index);
        let in_stream = input_format_context.streams().get(video_index).unwrap();

        let dd = in_stream.codecpar();

        let cc = dd.deref();

        out_stream.set_codecpar(cc.clone());
        {
            // let aa = in_stream.metadata().as_deref().unwrap();

            // let vv = aa.clone();

            // out_stream.set_metadata(Some(vv));
        }
    }

    // output_format_context.dump(0, output_file)?;
    output_format_context.write_header()?;

    loop {
        let mut packet = match input_format_context.read_packet() {
            Ok(Some(x)) => x,
            // No more frames
            Ok(None) => break,
            Err(e) => bail!("Read frame error: {:?}", e),
        };

        // println!(
        //     "packet.stream_index.try_into().unwrap() {}",
        //     packet.stream_index
        // );

        let in_stream_time = input_format_context
            .streams()
            .get(packet.stream_index.try_into().unwrap())
            .unwrap()
            .time_base;

        let out_stream_time = output_format_context
            .streams()
            .get(packet.stream_index.try_into().unwrap())
            .unwrap()
            .time_base;

        packet.rescale_ts(in_stream_time, out_stream_time);

        packet.set_pts(av_rescale_q_rnd(
            packet.pts,
            in_stream_time,
            out_stream_time,
            ffi::AVRounding_AV_ROUND_NEAR_INF | ffi::AVRounding_AV_ROUND_PASS_MINMAX,
        ));
        packet.set_dts(av_rescale_q_rnd(
            packet.dts,
            in_stream_time,
            out_stream_time,
            ffi::AVRounding_AV_ROUND_NEAR_INF | ffi::AVRounding_AV_ROUND_PASS_MINMAX,
        ));
        packet.set_duration(av_rescale_q(
            packet.duration,
            in_stream_time,
            out_stream_time,
        ));
        packet.set_pos(-1);

        match output_format_context.interleaved_write_frame(&mut packet) {
            Ok(()) => Ok(()),
            Err(RsmpegError::InterleavedWriteFrameError(-22)) => Ok(()),
            Err(e) => Err(e),
        }
        .context("Interleaved write frame failed.")?;
    }

    output_format_context.write_trailer()?;
    Ok(())
}
