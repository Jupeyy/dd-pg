use std::{num::NonZeroUsize, sync::Arc};

use av_data::{
    params::CodecParams, pixel::FromPrimitive, rational::Rational64, timeinfo::TimeInfo,
};
use av_format::{common::GlobalInfo, stream::Stream};
use base_log::log::{LogLevel, SystemLog, SystemLogGroup, SystemLogInterface};
use matroska::muxer::MkvMuxer;
use rav1e::{
    prelude::{FrameType, SpeedSettings},
    Config, Context, EncoderConfig, EncoderStatus,
};
use threadpool::ThreadPool;

pub struct AudioVideoEncoder {
    ctx: Context<u8>,
    enc_cfg: EncoderConfig,
    _thread_pool: ThreadPool,
    cur_frame: usize,
    logger: SystemLogGroup,
    muxer: av_format::muxer::Context<MkvMuxer, Vec<u8>>,
}

impl AudioVideoEncoder {
    pub fn new(log: &SystemLog) -> Self {
        let enc_cfg = EncoderConfig {
            width: 64,
            height: 96,
            speed_settings: SpeedSettings::from_preset(9),
            bit_depth: 8,
            ..Default::default()
        };

        let cfg = Config::new().with_encoder_config(enc_cfg.clone());

        let ctx: Context<u8> = cfg.new_context().unwrap();

        let mut muxer = av_format::muxer::Context::new(
            MkvMuxer::webm(),
            av_format::muxer::Writer::new(Vec::new()),
        );
        muxer.configure().unwrap();
        muxer
            .set_global_info(GlobalInfo {
                duration: None,
                timebase: None,
                streams: vec![Stream::from_params(
                    &CodecParams {
                        kind: None,
                        codec_id: None,
                        extradata: None,
                        bit_rate: 4096,
                        convergence_window: 0,
                        delay: 0,
                    },
                    Rational64::from_f64(0.0).unwrap(),
                )],
            })
            .unwrap();
        muxer.write_header().unwrap();

        Self {
            ctx,
            enc_cfg,
            _thread_pool: threadpool::Builder::new()
                .thread_name("av-encoder".to_string())
                .num_threads(
                    std::thread::available_parallelism()
                        .unwrap_or(NonZeroUsize::new(2).unwrap())
                        .get(),
                )
                .build(),
            cur_frame: 0,
            logger: log.logger("av-encoder"),
            muxer,
        }
    }

    pub fn next_video_frame(&mut self, pixel_buffer: Vec<u8>) {
        let ctx = &mut self.ctx;

        let mut f = ctx.new_frame();

        for p in &mut f.planes {
            let stride = (self.enc_cfg.width + p.cfg.xdec) >> p.cfg.xdec;
            p.copy_from_raw_u8(&pixel_buffer, stride, 1);
        }

        match ctx.send_frame(f.clone()) {
            Ok(_) => {}
            Err(e) => match e {
                EncoderStatus::EnoughData => {
                    self.logger
                        .log(LogLevel::Info)
                        .msg("Unable to append frame ")
                        .msg_var(&self.cur_frame)
                        .msg(" to the internal queue");
                }
                _ => {
                    panic!("Unable to send frame {}", self.cur_frame);
                }
            },
        }

        match ctx.receive_packet() {
            Ok(pkt) => {
                self.logger
                    .log(LogLevel::Info)
                    .msg("Packet ")
                    .msg_var(&pkt.input_frameno);
                self.cur_frame += 1;
                let mut write_pkt = av_data::packet::Packet::new();
                write_pkt.data = pkt.data;
                write_pkt.pos = Some(pkt.input_frameno as usize);
                write_pkt.stream_index = 0;
                write_pkt.t = TimeInfo {
                    pts: None,
                    dts: None,
                    duration: None,
                    timebase: None,
                    user_private: None,
                };
                write_pkt.is_key = if let FrameType::KEY = pkt.frame_type {
                    true
                } else {
                    false
                };
                write_pkt.is_corrupted = false;

                self.muxer.write_packet(Arc::new(write_pkt)).unwrap();
            }
            Err(e) => match e {
                EncoderStatus::LimitReached => {
                    self.logger.log(LogLevel::Info).msg("Limit reached");
                    todo!("this should never be called and indicates a bug, should we ignore it though?");
                }
                EncoderStatus::Encoded => {
                    self.logger.log(LogLevel::Info).msg("  Encoded");
                }
                EncoderStatus::NeedMoreData => {
                    self.logger.log(LogLevel::Info).msg("  Need more data");
                }
                _ => {
                    panic!("Unable to receive packet {}", self.cur_frame);
                }
            },
        }
    }

    pub fn finish_and_destroy(mut self) {
        self.ctx.flush();
    }
}
