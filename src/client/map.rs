use crate::{
    client_map_buffered::ClientMapBuffered,
    datafile::CDatafileWrapper,
    game::state::GameStateInterface,
    mapdef::{
        CEnvPoint, CMapItemGroup, CMapItemLayerQuads, CMapItemLayerTilemap, CQuad, CSpeedupTile,
        CSwitchTile, CTeleTile, CTile, CTuneTile, LayerFlag, MapLayer, MapLayerTile, MapLayerTypes,
    },
};

use math::math::{mix, vector::vec2, PI};

use base::system::SystemInterface;

use super::{
    render_pipe::RenderPipeline,
    render_tools::{LayerRenderFlag, RenderTools, TileRenderFlag},
};

use graphics_types::{
    command_buffer::{BufferContainerIndex, SQuadRenderInfo, GRAPHICS_MAX_QUADS_RENDER_COUNT},
    rendering::{ColorRGBA, State},
};

#[derive(PartialEq, PartialOrd)]
enum RenderMapTypes {
    TYPE_BACKGROUND = 0,
    TYPE_BACKGROUND_FORCE,
    TYPE_FOREGROUND,
    TYPE_FULL_DESIGN,
    TYPE_ALL = -1,
}

pub struct RenderMap {
    map_type: RenderMapTypes,
}

impl RenderMap {
    pub fn new() -> RenderMap {
        RenderMap {
            map_type: RenderMapTypes::TYPE_FULL_DESIGN,
        }
    }

    fn EnvelopeEval(
        &self,
        map: &CDatafileWrapper,
        game: &dyn GameStateInterface,
        sys: &dyn SystemInterface,
        TimeOffsetMillis: i32,
        Env: i32,
        Channels: &mut ColorRGBA,
    ) {
        *Channels = ColorRGBA::default();

        let mut pPoints: Option<&[CEnvPoint]> = None;

        {
            let Num = map.env_point_count();
            if Num > 0 {
                pPoints = Some(map.get_env_points()[0].as_slice());
            }
        }

        let Num = map.env_count();

        if Env as usize >= Num {
            return;
        }

        let pItem = &map.get_env(Env as usize);

        let TickToNanoSeconds =
            std::time::Duration::from_secs(0).as_nanos() as u64 / game.game_tick_speed() as u64;

        let mut s_Time = std::time::Duration::from_nanos(0);
        let mut s_LastLocalTime = sys.time_get_nanoseconds();

        if pItem.version < 2 || pItem.synchronized > 0 {
            // get the lerp of the current tick and prev
            let MinTick = game.game_tick() - game.game_start_tick();
            let CurTick = game.game_tick() - game.game_start_tick();
            s_Time = std::time::Duration::from_nanos(
                (mix::<f64, f64>(&0.0, &((CurTick - MinTick) as f64), game.intra_tick(sys))
                    * TickToNanoSeconds as f64) as u64
                    + MinTick * TickToNanoSeconds,
            );
        } else {
            let CurTime = sys.time_get_nanoseconds();
            s_Time += CurTime - s_LastLocalTime;
            s_LastLocalTime = CurTime;
        }
        RenderTools::render_eval_envelope(
            pPoints.unwrap().split_at(pItem.start_point as usize).1,
            pItem.num_points,
            4,
            s_Time + std::time::Duration::from_millis(TimeOffsetMillis as u64),
            Channels,
        );
    }

    fn RenderTileLayer(
        &self,
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        LayerIndex: usize,
        mut Color: ColorRGBA,
        pTileLayer: &CMapItemLayerTilemap,
        pGroup: &CMapItemGroup,
    ) {
        let Visuals = &buffered_map.tile_layer_visuals[LayerIndex];
        if Visuals.buffer_container_index.is_none() {
            return; //no visuals were created
        }

        let (mut ScreenX0, mut ScreenY0, mut ScreenX1, mut ScreenY1) = (0.0, 0.0, 0.0, 0.0);
        state.get_canvas_mapping(&mut ScreenX0, &mut ScreenY0, &mut ScreenX1, &mut ScreenY1);

        let mut Channels = ColorRGBA {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        if pTileLayer.color_env >= 0 {
            self.EnvelopeEval(
                pipe.map,
                pipe.game,
                pipe.sys,
                pTileLayer.color_env_offset,
                pTileLayer.color_env,
                &mut Channels,
            );
        }

        let mut BorderX0 = 0;
        let mut BorderY0 = 0;
        let mut BorderX1 = 0;
        let mut BorderY1 = 0;
        let mut DrawBorder = false;

        BorderY0 = (ScreenY0 / 32.0).floor() as i32;
        BorderX0 = (ScreenX0 / 32.0).floor() as i32;
        BorderY1 = (ScreenY1 / 32.0).floor() as i32;
        BorderX1 = (ScreenX1 / 32.0).floor() as i32;

        let mut Y0 = BorderY0;
        let mut X0 = BorderX0;
        let mut Y1 = BorderY1;
        let mut X1 = BorderX1;

        if X0 <= 0 {
            X0 = 0;
            DrawBorder = true;
        }
        if Y0 <= 0 {
            Y0 = 0;
            DrawBorder = true;
        }
        if X1 >= pTileLayer.width - 1 {
            X1 = pTileLayer.width - 1;
            DrawBorder = true;
        }
        if Y1 >= pTileLayer.height - 1 {
            Y1 = pTileLayer.height - 1;
            DrawBorder = true;
        }

        let mut DrawLayer = true;
        if X1 < 0 {
            DrawLayer = false;
        }
        if Y1 < 0 {
            DrawLayer = false;
        }
        if X0 >= pTileLayer.width {
            DrawLayer = false;
        }
        if Y0 >= pTileLayer.height {
            DrawLayer = false;
        }

        if DrawLayer {
            // indices buffers we want to draw
            // TODO: reuse them
            let mut s_vpIndexOffsets: Vec<usize> = Vec::new();
            let mut s_vDrawCounts: Vec<usize> = Vec::new();

            s_vpIndexOffsets.clear();
            s_vDrawCounts.clear();

            let Reserve: usize = (Y1 - Y0).abs() as usize + 1;
            s_vpIndexOffsets.reserve(Reserve);
            s_vDrawCounts.reserve(Reserve);

            for y in Y0..=Y1 {
                if X0 > X1 {
                    continue;
                }

                if Visuals.tiles_of_layer[(y * pTileLayer.width + X1) as usize]
                    .index_buffer_byte_offset()
                    < Visuals.tiles_of_layer[(y * pTileLayer.width + X0) as usize]
                        .index_buffer_byte_offset()
                {
                    panic!("Tile count wrong.");
                }

                let NumVertices = ((Visuals.tiles_of_layer[(y * pTileLayer.width + X1) as usize]
                    .index_buffer_byte_offset()
                    - Visuals.tiles_of_layer[(y * pTileLayer.width + X0) as usize]
                        .index_buffer_byte_offset())
                    / std::mem::size_of::<u32>())
                    + (if Visuals.tiles_of_layer[(y * pTileLayer.width + X1) as usize].do_draw() {
                        6
                    } else {
                        0
                    });

                if NumVertices > 0 {
                    s_vpIndexOffsets.push(
                        Visuals.tiles_of_layer[(y * pTileLayer.width + X0) as usize]
                            .index_buffer_byte_offset(),
                    );
                    s_vDrawCounts.push(NumVertices);
                }
            }

            Color.r *= Channels.r;
            Color.g *= Channels.g;
            Color.b *= Channels.b;
            Color.a *= Channels.a;

            let DrawCount = s_vpIndexOffsets.len();
            if DrawCount != 0 {
                pipe.graphics.RenderTileLayer(
                    state,
                    &Visuals.buffer_container_index,
                    &Color,
                    s_vpIndexOffsets.clone(), // TODO: heap alloc
                    s_vDrawCounts.clone(),    // TODO: heap alloc
                    DrawCount,
                );
            }
        }

        if DrawBorder {
            self.RenderTileBorder(
                buffered_map,
                state,
                pipe,
                LayerIndex,
                &Color,
                pTileLayer,
                pGroup,
                BorderX0,
                BorderY0,
                BorderX1,
                BorderY1,
                (-((-ScreenX1) / 32.0).floor()) as i32 - BorderX0,
                (-((-ScreenY1) / 32.0).floor()) as i32 - BorderY0,
            );
        }
    }

    fn RenderTileBorderCornerTiles(
        &self,
        _buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        WidthOffsetToOrigin: i32,
        HeightOffsetToOrigin: i32,
        TileCountWidth: i32,
        TileCountHeight: i32,
        buffer_container_index: BufferContainerIndex,
        Color: &ColorRGBA,
        IndexBufferOffset: usize,
        Offset: &vec2,
        dir: &vec2,
    ) {
        // if border is still in range of the original corner, it doesn't needs to be redrawn
        let CornerVisible = (WidthOffsetToOrigin - 1 < TileCountWidth)
            && (HeightOffsetToOrigin - 1 < TileCountHeight);

        let CountX = WidthOffsetToOrigin.min(TileCountWidth);
        let CountY = HeightOffsetToOrigin.min(TileCountHeight);

        let Count = (CountX * CountY) as usize - (if CornerVisible { 1 } else { 0 }); // Don't draw the corner again

        pipe.graphics.RenderBorderTiles(
            state,
            &buffer_container_index,
            Color,
            IndexBufferOffset,
            Offset,
            dir,
            CountX,
            Count,
        );
    }

    fn RenderTileBorder(
        &self,
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        LayerIndex: usize,
        Color: &ColorRGBA,
        pTileLayer: &CMapItemLayerTilemap,
        _pGroup: &CMapItemGroup,
        BorderX0: i32,
        BorderY0: i32,
        BorderX1: i32,
        BorderY1: i32,
        ScreenWidthTileCount: i32,
        ScreenHeightTileCount: i32,
    ) {
        let Visuals = &buffered_map.tile_layer_visuals[LayerIndex];

        let mut Y0 = BorderY0;
        let mut X0 = BorderX0;
        let mut Y1 = BorderY1;
        let mut X1 = BorderX1;

        let CountWidth = ScreenWidthTileCount;
        let CountHeight = ScreenHeightTileCount;

        if X0 < 1 {
            X0 = 1;
        }
        if Y0 < 1 {
            Y0 = 1;
        }
        if X1 >= pTileLayer.width - 1 {
            X1 = pTileLayer.width - 2;
        }
        if Y1 >= pTileLayer.height - 1 {
            Y1 = pTileLayer.height - 2;
        }

        if BorderX0 <= 0 {
            // Draw corners on left side
            if BorderY0 <= 0 {
                if Visuals.border_top_left.do_draw() {
                    let mut Offset = vec2::default();
                    Offset.x = BorderX0 as f32 * 32.0;
                    Offset.y = BorderY0 as f32 * 32.0;
                    let mut dir = vec2::default();
                    dir.x = 32.0;
                    dir.y = 32.0;

                    self.RenderTileBorderCornerTiles(
                        buffered_map,
                        state,
                        pipe,
                        (BorderX0).abs() + 1,
                        (BorderY0).abs() + 1,
                        CountWidth,
                        CountHeight,
                        Visuals.buffer_container_index,
                        Color,
                        Visuals.border_top_left.index_buffer_byte_offset(),
                        &Offset,
                        &dir,
                    );
                }
            }
            if BorderY1 >= pTileLayer.height - 1 {
                if Visuals.border_bottom_left.do_draw() {
                    let mut Offset = vec2::default();
                    Offset.x = BorderX0 as f32 * 32.0;
                    Offset.y = (BorderY1 - (pTileLayer.height - 1)) as f32 * 32.0;
                    let mut dir = vec2::default();
                    dir.x = 32.0;
                    dir.y = -32.0;

                    self.RenderTileBorderCornerTiles(
                        buffered_map,
                        state,
                        pipe,
                        (BorderX0).abs() + 1,
                        (BorderY1 - (pTileLayer.height - 1)) + 1,
                        CountWidth,
                        CountHeight,
                        Visuals.buffer_container_index,
                        Color,
                        Visuals.border_bottom_left.index_buffer_byte_offset(),
                        &Offset,
                        &dir,
                    );
                }
            }
        }
        if BorderX0 < 0 {
            // Draw left border
            if Y0 < pTileLayer.height - 1 && Y1 > 0 {
                let DrawNum = (((Visuals.border_left[(Y1 - 1) as usize]
                    .index_buffer_byte_offset()
                    - Visuals.border_left[(Y0 - 1) as usize].index_buffer_byte_offset())
                    / std::mem::size_of::<u32>())
                    + (if Visuals.border_left[(Y1 - 1) as usize].do_draw() {
                        6
                    } else {
                        0
                    })) as usize;
                let pOffset = Visuals.border_left[(Y0 - 1) as usize].index_buffer_byte_offset();
                let mut Offset = vec2::default();
                Offset.x = 32.0 * BorderX0 as f32;
                Offset.y = 0.0;
                let mut dir = vec2::default();
                dir.x = 32.0;
                dir.y = 0.0;
                pipe.graphics.RenderBorderTileLines(
                    state,
                    &Visuals.buffer_container_index,
                    Color,
                    pOffset,
                    &Offset,
                    &dir,
                    DrawNum,
                    BorderX0.abs().min(CountWidth) as usize,
                );
            }
        }

        if BorderX1 >= pTileLayer.width - 1 {
            // Draw corners on right side
            if BorderY0 <= 0 {
                if Visuals.border_top_right.do_draw() {
                    let mut Offset = vec2::default();
                    Offset.x = (BorderX1 - (pTileLayer.width - 1)) as f32 * 32.0;
                    Offset.y = BorderY0 as f32 * 32.0;
                    let mut dir = vec2::default();
                    dir.x = -32.0;
                    dir.y = 32.0;

                    self.RenderTileBorderCornerTiles(
                        buffered_map,
                        state,
                        pipe,
                        (BorderX1 - (pTileLayer.width - 1)) + 1,
                        (BorderY0.abs()) + 1,
                        CountWidth,
                        CountHeight,
                        Visuals.buffer_container_index,
                        Color,
                        Visuals.border_top_right.index_buffer_byte_offset(),
                        &Offset,
                        &dir,
                    );
                }
            }
            if BorderY1 >= pTileLayer.height - 1 {
                if Visuals.border_bottom_right.do_draw() {
                    let mut Offset = vec2::default();
                    Offset.x = (BorderX1 - (pTileLayer.width - 1)) as f32 * 32.0;
                    Offset.y = (BorderY1 - (pTileLayer.height - 1)) as f32 * 32.0;
                    let mut dir = vec2::default();
                    dir.x = -32.0;
                    dir.y = -32.0;

                    self.RenderTileBorderCornerTiles(
                        buffered_map,
                        state,
                        pipe,
                        (BorderX1 - (pTileLayer.width - 1)) + 1,
                        (BorderY1 - (pTileLayer.height - 1)) + 1,
                        CountWidth,
                        CountHeight,
                        Visuals.buffer_container_index,
                        Color,
                        Visuals.border_bottom_right.index_buffer_byte_offset(),
                        &Offset,
                        &dir,
                    );
                }
            }
        }
        if BorderX1 > pTileLayer.width - 1 {
            // Draw right border
            if Y0 < pTileLayer.height - 1 && Y1 > 0 {
                let DrawNum = ((Visuals.border_right[(Y1 - 1) as usize]
                    .index_buffer_byte_offset()
                    - Visuals.border_right[(Y0 - 1) as usize].index_buffer_byte_offset())
                    / std::mem::size_of::<u32>())
                    + (if Visuals.border_right[(Y1 - 1) as usize].do_draw() {
                        6
                    } else {
                        0
                    });
                let pOffset = Visuals.border_right[(Y0 - 1) as usize].index_buffer_byte_offset();
                let mut Offset = vec2::default();
                Offset.x = 32.0 * (BorderX1 - (pTileLayer.width - 1)) as f32;
                Offset.y = 0.0;
                let mut dir = vec2::default();
                dir.x = -32.0;
                dir.y = 0.0;
                pipe.graphics.RenderBorderTileLines(
                    state,
                    &Visuals.buffer_container_index,
                    Color,
                    pOffset,
                    &Offset,
                    &dir,
                    DrawNum,
                    (BorderX1 - (pTileLayer.width - 1)).min(CountWidth) as usize,
                );
            }
        }
        if BorderY0 < 0 {
            // Draw top border
            if X0 < pTileLayer.width - 1 && X1 > 0 {
                let DrawNum = ((Visuals.border_top[(X1 - 1) as usize].index_buffer_byte_offset()
                    - Visuals.border_top[(X0 - 1) as usize].index_buffer_byte_offset())
                    / std::mem::size_of::<u32>())
                    + (if Visuals.border_top[(X1 - 1) as usize].do_draw() {
                        6
                    } else {
                        0
                    });
                let pOffset = Visuals.border_top[(X0 - 1) as usize].index_buffer_byte_offset();
                let mut Offset = vec2::default();
                Offset.x = 0.0;
                Offset.y = 32.0 * BorderY0 as f32;
                let mut dir = vec2::default();
                dir.x = 0.0;
                dir.y = 32.0;
                pipe.graphics.RenderBorderTileLines(
                    state,
                    &Visuals.buffer_container_index,
                    Color,
                    pOffset,
                    &Offset,
                    &dir,
                    DrawNum,
                    (BorderY0).abs().min(CountHeight) as usize,
                );
            }
        }
        if BorderY1 >= pTileLayer.height {
            // Draw bottom border
            if X0 < pTileLayer.width - 1 && X1 > 0 {
                let DrawNum = ((Visuals.border_bottom[(X1 - 1) as usize]
                    .index_buffer_byte_offset()
                    - Visuals.border_bottom[(X0 - 1) as usize].index_buffer_byte_offset())
                    / std::mem::size_of::<u32>())
                    + (if Visuals.border_bottom[(X1 - 1) as usize].do_draw() {
                        6
                    } else {
                        0
                    });
                let pOffset = Visuals.border_bottom[(X0 - 1) as usize].index_buffer_byte_offset();
                let mut Offset = vec2::default();
                Offset.x = 0.0;
                Offset.y = 32.0 * (BorderY1 - (pTileLayer.height - 1)) as f32;
                let mut dir = vec2::default();
                dir.x = 0.0;
                dir.y = -32.0;
                pipe.graphics.RenderBorderTileLines(
                    state,
                    &Visuals.buffer_container_index,
                    Color,
                    pOffset,
                    &Offset,
                    &dir,
                    DrawNum,
                    (BorderY1 - (pTileLayer.height - 1)).min(CountHeight) as usize,
                );
            }
        }
    }

    fn RenderKillTileBorder(
        &self,
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        LayerIndex: usize,
        Color: &ColorRGBA,
        pTileLayer: &CMapItemLayerTilemap,
        _pGroup: &CMapItemGroup,
    ) {
        let Visuals = &buffered_map.tile_layer_visuals[LayerIndex];
        if Visuals.buffer_container_index.is_none() {
            return; //no visuals were created
        }

        let (mut ScreenX0, mut ScreenY0, mut ScreenX1, mut ScreenY1) = (0.0, 0.0, 0.0, 0.0);
        state.get_canvas_mapping(&mut ScreenX0, &mut ScreenY0, &mut ScreenX1, &mut ScreenY1);

        let mut DrawBorder = false;

        let mut BorderY0 = (ScreenY0 / 32.0) as i32 - 1;
        let mut BorderX0 = (ScreenX0 / 32.0) as i32 - 1;
        let mut BorderY1 = (ScreenY1 / 32.0) as i32 + 1;
        let mut BorderX1 = (ScreenX1 / 32.0) as i32 + 1;

        if BorderX0 < -201 {
            DrawBorder = true;
        }
        if BorderY0 < -201 {
            DrawBorder = true;
        }
        if BorderX1 >= pTileLayer.width + 201 {
            DrawBorder = true;
        }
        if BorderY1 >= pTileLayer.height + 201 {
            DrawBorder = true;
        }

        if !DrawBorder {
            return;
        }
        if !Visuals.border_kill_tile.do_draw() {
            return;
        }

        if BorderX0 < -300 {
            BorderX0 = -300;
        }
        if BorderY0 < -300 {
            BorderY0 = -300;
        }
        if BorderX1 >= pTileLayer.width + 300 {
            BorderX1 = pTileLayer.width + 299;
        }
        if BorderY1 >= pTileLayer.height + 300 {
            BorderY1 = pTileLayer.height + 299;
        }

        if BorderX1 < -300 {
            BorderX1 = -300;
        }
        if BorderY1 < -300 {
            BorderY1 = -300;
        }
        if BorderX0 >= pTileLayer.width + 300 {
            BorderX0 = pTileLayer.width + 299;
        }
        if BorderY0 >= pTileLayer.height + 300 {
            BorderY0 = pTileLayer.height + 299;
        }

        // Draw left kill tile border
        if BorderX0 < -201 {
            let mut Offset = vec2::default();
            Offset.x = BorderX0 as f32 * 32.0;
            Offset.y = BorderY0 as f32 * 32.0;
            let mut dir = vec2::default();
            dir.x = 32.0;
            dir.y = 32.0;

            let Count = ((BorderX0).abs() - 201) * (BorderY1 - BorderY0);

            pipe.graphics.RenderBorderTiles(
                state,
                &Visuals.buffer_container_index,
                Color,
                Visuals.border_kill_tile.index_buffer_byte_offset(),
                &Offset,
                &dir,
                (BorderX0).abs() - 201,
                Count as usize,
            );
        }
        // Draw top kill tile border
        if BorderY0 < -201 {
            let mut Offset = vec2::default();
            let mut OffX0 = if BorderX0 < -201 { -201 } else { BorderX0 };
            let mut OffX1 = if BorderX1 >= pTileLayer.width + 201 {
                pTileLayer.width + 201
            } else {
                BorderX1
            };
            OffX0 = OffX0.clamp(-201, pTileLayer.width + 201);
            OffX1 = OffX1.clamp(-201, pTileLayer.width + 201);
            Offset.x = OffX0 as f32 * 32.0;
            Offset.y = BorderY0 as f32 * 32.0;
            let mut dir = vec2::default();
            dir.x = 32.0;
            dir.y = 32.0;

            let Count = (OffX1 - OffX0) * ((BorderY0).abs() - 201);

            pipe.graphics.RenderBorderTiles(
                state,
                &Visuals.buffer_container_index,
                Color,
                Visuals.border_kill_tile.index_buffer_byte_offset(),
                &Offset,
                &dir,
                OffX1 - OffX0,
                Count as usize,
            );
        }
        if BorderX1 >= pTileLayer.width + 201 {
            let mut Offset = vec2::default();
            Offset.x = (pTileLayer.width + 201) as f32 * 32.0;
            Offset.y = BorderY0 as f32 * 32.0;
            let mut dir = vec2::default();
            dir.x = 32.0;
            dir.y = 32.0;

            let Count = (BorderX1 - (pTileLayer.width + 201)) * (BorderY1 - BorderY0);

            pipe.graphics.RenderBorderTiles(
                state,
                &Visuals.buffer_container_index,
                Color,
                Visuals.border_kill_tile.index_buffer_byte_offset(),
                &Offset,
                &dir,
                BorderX1 - (pTileLayer.width + 201),
                Count as usize,
            );
        }
        if BorderY1 >= pTileLayer.height + 201 {
            let mut Offset = vec2::default();
            let mut OffX0 = if BorderX0 < -201 { -201 } else { BorderX0 };
            let mut OffX1 = if BorderX1 >= pTileLayer.width + 201 {
                pTileLayer.width + 201
            } else {
                BorderX1
            };
            OffX0 = OffX0.clamp(-201, pTileLayer.width + 201);
            OffX1 = OffX1.clamp(-201, pTileLayer.width + 201);
            Offset.x = OffX0 as f32 * 32.0;
            Offset.y = (pTileLayer.height + 201) as f32 * 32.0;
            let mut dir = vec2::default();
            dir.x = 32.0;
            dir.y = 32.0;

            let Count = (OffX1 - OffX0) * (BorderY1 - (pTileLayer.height + 201));

            pipe.graphics.RenderBorderTiles(
                state,
                &Visuals.buffer_container_index,
                Color,
                Visuals.border_kill_tile.index_buffer_byte_offset(),
                &Offset,
                &dir,
                OffX1 - OffX0,
                Count as usize,
            );
        }
    }

    fn RenderQuadLayer(
        &self,
        buffered_map: &ClientMapBuffered,
        state: &State,
        pipe: &mut RenderPipeline,
        LayerIndex: usize,
        pQuadLayer: &CMapItemLayerQuads,
        pQuads: &Vec<CQuad>,
        _pGroup: &CMapItemGroup,
        Force: bool,
    ) {
        let Visuals = &buffered_map.quad_layer_visuals[LayerIndex];
        if Visuals.buffer_container_index.is_none() {
            return; //no visuals were created
        }

        if !Force
        // TODO: && (!g_Config.m_ClShowQuads || g_Config.m_ClOverlayEntities == 100)) {
            && false
        {
            return;
        }

        let mut s_vQuadRenderInfo: Vec<SQuadRenderInfo> = Vec::new();

        s_vQuadRenderInfo.resize(pQuadLayer.num_quads as usize, Default::default());
        let mut QuadsRenderCount = 0;
        let mut CurQuadOffset = 0;
        for i in 0..pQuadLayer.num_quads as usize {
            let pQuad = &pQuads[i];

            let mut Color = ColorRGBA {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            if pQuad.color_env >= 0 {
                self.EnvelopeEval(
                    pipe.map,
                    pipe.game,
                    pipe.sys,
                    pQuad.color_env_offset,
                    pQuad.color_env,
                    &mut Color,
                );
            }

            let mut OffsetX = 0.0;
            let mut OffsetY = 0.0;
            let mut Rot = 0.0;

            if pQuad.pos_env >= 0 {
                let mut Channels = ColorRGBA::default();
                self.EnvelopeEval(
                    pipe.map,
                    pipe.game,
                    pipe.sys,
                    pQuad.pos_env_offset,
                    pQuad.pos_env,
                    &mut Channels,
                );
                OffsetX = Channels.r;
                OffsetY = Channels.g;
                Rot = Channels.b / 180.0 * PI;
            }

            let IsFullyTransparent = Color.a <= 0.0;
            let NeedsFlush =
                QuadsRenderCount == GRAPHICS_MAX_QUADS_RENDER_COUNT || IsFullyTransparent;

            if NeedsFlush {
                // render quads of the current offset directly(cancel batching)
                s_vQuadRenderInfo.resize(QuadsRenderCount, Default::default());
                pipe.graphics.RenderQuadLayer(
                    &state,
                    &Visuals.buffer_container_index,
                    s_vQuadRenderInfo.clone(),
                    QuadsRenderCount,
                    CurQuadOffset,
                );
                s_vQuadRenderInfo.resize(pQuadLayer.num_quads as usize, Default::default());
                QuadsRenderCount = 0;
                CurQuadOffset = i;
                if IsFullyTransparent {
                    // since this quad is ignored, the offset is the next quad
                    CurQuadOffset += 1;
                }
            }

            if !IsFullyTransparent {
                let QInfo = &mut s_vQuadRenderInfo[QuadsRenderCount];
                QuadsRenderCount += 1;
                QInfo.color = Color;
                QInfo.offsets.x = OffsetX;
                QInfo.offsets.y = OffsetY;
                QInfo.rotation = Rot;
            }
        }
        s_vQuadRenderInfo.resize(QuadsRenderCount, Default::default());
        pipe.graphics.RenderQuadLayer(
            &state,
            &Visuals.buffer_container_index,
            s_vQuadRenderInfo,
            QuadsRenderCount,
            CurQuadOffset,
        );
    }

    fn LayersOfGroupCount(
        &self,
        pipe: &mut RenderPipeline,
        pGroup: &CMapItemGroup,
        TileLayerCount: &mut usize,
        QuadLayerCount: &mut usize,
        PassedGameLayer: &mut bool,
    ) {
        let mut TileLayerCounter = 0;
        let mut QuadLayerCounter = 0;
        for l in 0..pGroup.num_layers as usize {
            let layer_index = pGroup.start_layer as usize + l;
            let pLayer = pipe.map.get_layer(layer_index);
            let mut IsFrontLayer = false;
            let mut IsSwitchLayer = false;
            let mut IsTeleLayer = false;
            let mut IsSpeedupLayer = false;
            let mut IsTuneLayer = false;

            if pipe.map.is_game_layer(layer_index) {
                *PassedGameLayer = true;
            }

            if pipe.map.is_front_layer(layer_index) {
                IsFrontLayer = true;
            }

            if pipe.map.is_switch_layer(layer_index) {
                IsSwitchLayer = true;
            }

            if pipe.map.is_tele_layer(layer_index) {
                IsTeleLayer = true;
            }

            if pipe.map.is_speedup_layer(layer_index) {
                IsSpeedupLayer = true;
            }

            if pipe.map.is_tune_layer(layer_index) {
                IsTuneLayer = true;
            }

            /*if(m_Type <= TYPE_BACKGROUND_FORCE)
            {
                if(PassedGameLayer)
                    break;
            }
            else if(m_Type == TYPE_FOREGROUND)
            {
                if(!PassedGameLayer)
                    continue;
            }*/

            if let MapLayer::Tile(_pTMap) = pLayer {
                let mut TileLayerAndOverlayCount = 0;
                if IsFrontLayer {
                    TileLayerAndOverlayCount = 1;
                } else if IsSwitchLayer {
                    TileLayerAndOverlayCount = 3;
                } else if IsTeleLayer {
                    TileLayerAndOverlayCount = 2;
                } else if IsSpeedupLayer {
                    TileLayerAndOverlayCount = 3;
                } else if IsTuneLayer {
                    TileLayerAndOverlayCount = 1;
                } else {
                    TileLayerAndOverlayCount = 1;
                }

                TileLayerCounter += TileLayerAndOverlayCount;
            } else if let MapLayer::Quads(_) = pLayer {
                QuadLayerCounter += 1;
            }
        }

        *TileLayerCount += TileLayerCounter;
        *QuadLayerCount += QuadLayerCounter;
    }

    pub fn render(&self, pipe: &mut RenderPipeline) {
        // TODO if m_OnlineOnly && Client().State() != IClient::STATE_ONLINE && Client().State() != IClient::STATE_DEMOPLAYBACK)
        //	return;

        let Center: vec2 = vec2 {
            x: pipe.camera.x,
            y: pipe.camera.y,
        };

        let mut PassedGameLayer = false;
        let mut TileLayerCounter: usize = 0;
        let mut QuadLayerCounter: usize = 0;

        let mut state = State::new();

        for g in 0..pipe.map.NumGroups() as usize {
            let pGroup = &pipe.map.get_group(g);
            let pGroupEx = None; //pipe.map.GetGroupEx(g);

            /* TODO filter group before? if pGroup.is_null()
            {
                dbg_msg("maplayers", "error group was null, group number = %d, total groups = %d", g, map.NumGroups());
                dbg_msg("maplayers", "this is here to prevent a crash but the source of this is unknown, please report this for it to get fixed");
                dbg_msg("maplayers", "we need mapname and crc and the map that caused this if possible, and anymore info you think is relevant");
                continue;
            }*/

            if (!pipe.config.gfx_no_clip || self.map_type == RenderMapTypes::TYPE_FULL_DESIGN)
                && (*pGroup).version >= 2
                && (*pGroup).use_clipping > 0
            {
                // set clipping
                RenderTools::map_canvas_to_group(
                    &pipe.graphics,
                    &mut state,
                    Center.x,
                    Center.y,
                    pipe.map.get_game_group(),
                    None, // TODO: pipe.map.GameGroupEx(),
                    pipe.camera.zoom,
                );
                let (mut ScreenX0, mut ScreenY0, mut ScreenX1, mut ScreenY1) = (0.0, 0.0, 0.0, 0.0);
                state.get_canvas_mapping(
                    &mut ScreenX0,
                    &mut ScreenY0,
                    &mut ScreenX1,
                    &mut ScreenY1,
                );
                let aPoints: [f32; 4] = [ScreenX0, ScreenY0, ScreenX1, ScreenY1];

                let x0 = (pGroup.clip_x as f32 - aPoints[0]) / (aPoints[2] - aPoints[0]);
                let y0 = (pGroup.clip_y as f32 - aPoints[1]) / (aPoints[3] - aPoints[1]);
                let x1 = ((pGroup.clip_x + pGroup.clip_w) as f32 - aPoints[0])
                    / (aPoints[2] - aPoints[0]);
                let y1 = ((pGroup.clip_y + pGroup.clip_h) as f32 - aPoints[1])
                    / (aPoints[3] - aPoints[1]);

                if x1 < 0.0 || x0 > 1.0 || y1 < 0.0 || y0 > 1.0 {
                    //check tile layer count of this group
                    self.LayersOfGroupCount(
                        pipe,
                        pGroup,
                        &mut TileLayerCounter,
                        &mut QuadLayerCounter,
                        &mut PassedGameLayer,
                    );
                    continue;
                }

                state.clip(
                    (x0 * pipe.graphics.canvas_width() as f32) as i32,
                    (y0 * pipe.graphics.canvas_height() as f32) as i32,
                    ((x1 - x0) * pipe.graphics.canvas_width() as f32) as u32,
                    ((y1 - y0) * pipe.graphics.canvas_height() as f32) as u32,
                );
            }

            RenderTools::map_canvas_to_group(
                &pipe.graphics,
                &mut state,
                Center.x,
                Center.y,
                pGroup,
                pGroupEx,
                1.0, /* TODO Zoom */
            );

            for l in 0..pGroup.num_layers as usize {
                let layer_index = pGroup.start_layer as usize + l;
                let pLayer = pipe.map.get_layer(layer_index);
                let mut Render = false;
                let mut IsGameLayer = false;
                let mut IsFrontLayer = false;
                let mut IsSwitchLayer = false;
                let mut IsTeleLayer = false;
                let mut IsSpeedupLayer = false;
                let mut IsTuneLayer = false;
                let mut IsEntityLayer = false;

                if pipe.map.is_game_layer(layer_index) {
                    IsEntityLayer = true;
                    IsGameLayer = true;
                    PassedGameLayer = true;
                }

                if pipe.map.is_front_layer(layer_index) {
                    IsEntityLayer = true;
                    IsFrontLayer = true;
                }

                if pipe.map.is_switch_layer(layer_index) {
                    IsEntityLayer = true;
                    IsSwitchLayer = true;
                }

                if pipe.map.is_tele_layer(layer_index) {
                    IsEntityLayer = true;
                    IsTeleLayer = true;
                }

                if pipe.map.is_speedup_layer(layer_index) {
                    IsEntityLayer = true;
                    IsSpeedupLayer = true;
                }

                if pipe.map.is_tune_layer(layer_index) {
                    IsEntityLayer = true;
                    IsTuneLayer = true;
                }

                if self.map_type == RenderMapTypes::TYPE_ALL {
                    Render = true;
                } else if self.map_type <= RenderMapTypes::TYPE_BACKGROUND_FORCE {
                    if PassedGameLayer {
                        return;
                    }
                    Render = true;

                    if self.map_type == RenderMapTypes::TYPE_BACKGROUND_FORCE {
                        if pLayer.get_tile_layer_base().item_layer
                            == MapLayerTypes::LAYERTYPE_TILES as i32
                            && !pipe.config.cl_background_show_tile_layers
                        {
                            continue;
                        }
                    }
                } else if self.map_type == RenderMapTypes::TYPE_FOREGROUND {
                    if PassedGameLayer && !IsGameLayer {
                        Render = true;
                    }
                } else if self.map_type == RenderMapTypes::TYPE_FULL_DESIGN {
                    if !IsGameLayer {
                        Render = true;
                    }
                }

                /*
                if Render && pLayer.m_Type == LAYERTYPE_TILES && Input().ModifierIsPressed() && Input().ShiftIsPressed() && Input().KeyPress(KEY_KP_0))
                {
                    CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                    CTile *pTiles = (CTile *)m_pLayers.Map().GetData(pTMap.m_Data);
                    CServerInfo CurrentServerInfo;
                    Client().GetServerInfo(&CurrentServerInfo);
                    char aFilename[IO_MAX_PATH_LENGTH];
                    str_format(aFilename, sizeof(aFilename), "dumps/tilelayer_dump_%s-%d-%d-%dx%d.txt", CurrentServerInfo.m_aMap, g, l, pTMap.m_Width, pTMap.m_Height);
                    IOHANDLE File = Storage().OpenFile(aFilename, IOFLAG_WRITE, IStorage::TYPE_SAVE);
                    if File)
                    {
                        for(int y = 0; y < pTMap.m_Height; y++)
                        {
                            for(int x = 0; x < pTMap.m_Width; x++)
                                io_write(File, &(pTiles[y * pTMap.m_Width + x].m_Index), sizeof(pTiles[y * pTMap.m_Width + x].m_Index));
                            io_write_newline(File);
                        }
                        io_close(File);
                    }
                }*/

                if let MapLayer::Tile(MapLayerTile(pTMap, _, _)) = pLayer {
                    if Render || IsGameLayer {
                        let mut DataIndex: i32 = 0;
                        let mut TileSize: u32 = 0;
                        let mut TileLayerAndOverlayCount: usize = 0;
                        if IsFrontLayer {
                            DataIndex = pTMap.front;
                            TileSize = std::mem::size_of::<CTile>() as u32;
                            TileLayerAndOverlayCount = 1;
                        } else if IsSwitchLayer {
                            DataIndex = pTMap.switch;
                            TileSize = std::mem::size_of::<CSwitchTile>() as u32;
                            TileLayerAndOverlayCount = 3;
                        } else if IsTeleLayer {
                            DataIndex = pTMap.tele;
                            TileSize = std::mem::size_of::<CTeleTile>() as u32;
                            TileLayerAndOverlayCount = 2;
                        } else if IsSpeedupLayer {
                            DataIndex = pTMap.speedup;
                            TileSize = std::mem::size_of::<CSpeedupTile>() as u32;
                            TileLayerAndOverlayCount = 3;
                        } else if IsTuneLayer {
                            DataIndex = pTMap.tune;
                            TileSize = std::mem::size_of::<CTuneTile>() as u32;
                            TileLayerAndOverlayCount = 1;
                        } else {
                            DataIndex = pTMap.data;
                            TileSize = std::mem::size_of::<CTile>() as u32;
                            TileLayerAndOverlayCount = 1;
                        }

                        TileLayerCounter += TileLayerAndOverlayCount;
                    }
                } else if Render
                    && pLayer.get_tile_layer_base().item_layer
                        == MapLayerTypes::LAYERTYPE_QUADS as i32
                {
                    QuadLayerCounter += 1;
                }

                // skip rendering if detail layers if not wanted, or is entity layer and we are a background map
                if ((pLayer.get_tile_layer_base().flags & LayerFlag::LAYERFLAG_DETAIL as i32) != 0
                    && (!pipe.config.gfx_high_detail
                        && !(self.map_type == RenderMapTypes::TYPE_FULL_DESIGN))
                    && !IsGameLayer)
                    || (self.map_type == RenderMapTypes::TYPE_BACKGROUND_FORCE && IsEntityLayer)
                    || (self.map_type == RenderMapTypes::TYPE_FULL_DESIGN && IsEntityLayer)
                {
                    continue;
                }

                let mut EntityOverlayVal = pipe.config.cl_overlay_entities;
                if self.map_type == RenderMapTypes::TYPE_FULL_DESIGN {
                    EntityOverlayVal = 0;
                }

                if (Render
                    && EntityOverlayVal < 100
                    && !IsGameLayer
                    && !IsFrontLayer
                    && !IsSwitchLayer
                    && !IsTeleLayer
                    && !IsSpeedupLayer
                    && !IsTuneLayer)
                    || (EntityOverlayVal > 0 && IsGameLayer)
                    || (self.map_type == RenderMapTypes::TYPE_BACKGROUND_FORCE)
                {
                    if let MapLayer::Tile(MapLayerTile(pTMap, _, tiles)) = pLayer {
                        if pTMap.image == -1 {
                            if !IsGameLayer {
                                state.clear_texture();
                            } else {
                                // TODO pipe.graphics.texture_set(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_GAME));
                            }
                        } else {
                            state.set_texture(pipe.map_images[pTMap.image as usize].texture_index);
                        }
                        let pTiles = tiles;
                        let mut Color = ColorRGBA {
                            r: pTMap.color.r() as f32 / 255.0,
                            g: pTMap.color.g() as f32 / 255.0,
                            b: pTMap.color.b() as f32 / 255.0,
                            a: pTMap.color.a() as f32 / 255.0,
                        };
                        if IsGameLayer && EntityOverlayVal > 0 {
                            Color = ColorRGBA {
                                r: pTMap.color.r() as f32 / 255.0,
                                g: pTMap.color.g() as f32 / 255.0,
                                b: pTMap.color.b() as f32 / 255.0,
                                a: pTMap.color.a() as f32 / 255.0 * EntityOverlayVal as f32 / 100.0,
                            };
                        } else if !IsGameLayer
                            && EntityOverlayVal > 0
                            && !(self.map_type == RenderMapTypes::TYPE_BACKGROUND_FORCE)
                        {
                            Color = ColorRGBA {
                                r: pTMap.color.r() as f32 / 255.0,
                                g: pTMap.color.g() as f32 / 255.0,
                                b: pTMap.color.b() as f32 / 255.0,
                                a: pTMap.color.a() as f32 / 255.0 * (100 - EntityOverlayVal) as f32
                                    / 100.0,
                            };
                        }
                        if let Some(buffered_map) = pipe.buffered_map {
                            state.blend_normal();
                            // draw kill tiles outside the entity clipping rectangle
                            if IsGameLayer {
                                // slow blinking to hint that it's not a part of the map
                                let Seconds = pipe.sys.time_get_nanoseconds().as_secs_f64();
                                let ColorHint = ColorRGBA {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 0.3
                                        + 0.7
                                            * (1.0
                                                + (2.0 * PI as f64 * Seconds / 3.0).sin() as f32)
                                            / 2.0,
                                };

                                let ColorKill = ColorRGBA {
                                    r: Color.r * ColorHint.r,
                                    g: Color.g * ColorHint.g,
                                    b: Color.b * ColorHint.b,
                                    a: Color.a * ColorHint.a,
                                };
                                self.RenderKillTileBorder(
                                    buffered_map,
                                    &state,
                                    pipe,
                                    TileLayerCounter - 1,
                                    &ColorKill,
                                    pTMap,
                                    pGroup,
                                );
                            }
                            self.RenderTileLayer(
                                buffered_map,
                                &state,
                                pipe,
                                TileLayerCounter - 1,
                                Color,
                                pTMap,
                                pGroup,
                            );
                        } else {
                            state.blend_normal();

                            // draw kill tiles outside the entity clipping rectangle
                            /*TODO if IsGameLayer
                            {
                                // slow blinking to hint that it's not a part of the map
                                double Seconds = time_get() / (double)time_freq();
                                ColorRGBA ColorHint = ColorRGBA(1.0, 1.0, 1.0, 0.3 + 0.7 * (1 + sin(2 * (double)pi * Seconds / 3)) / 2);

                                RenderTools().RenderTileRectangle(-201, -201, pTMap.m_Width + 402, pTMap.m_Height + 402,
                                    0, TILE_DEATH, // display air inside, death outside
                                    32.0, Color.v4() * ColorHint.v4(), TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT,
                                    EnvelopeEval, this, pTMap.m_ColorEnv, pTMap.m_ColorEnvOffset);
                            }*/

                            RenderTools::render_tile_map(
                                pipe,
                                &state,
                                pTiles,
                                pTMap.width,
                                pTMap.height,
                                32.0,
                                &Color,
                                TileRenderFlag::Extend as i32 | LayerRenderFlag::Transparent as i32,
                                |pipe, time_offset_millis, env, channels| {
                                    self.EnvelopeEval(
                                        pipe.map,
                                        pipe.game,
                                        pipe.sys,
                                        time_offset_millis,
                                        env,
                                        channels,
                                    )
                                },
                                pTMap.color_env,
                                pTMap.color_env_offset,
                            );
                        }
                    } else if let MapLayer::Quads(pQLayer) = pLayer {
                        if pQLayer.0.image == -1 {
                            state.clear_texture();
                        } else {
                            state.set_texture(
                                pipe.map_images[pQLayer.0.image as usize].texture_index,
                            );
                        }

                        let pQuads = &pQLayer.1;
                        if false
                        /*TODO: self.map_type == TYPE_BACKGROUND_FORCE
                        || self.map_type == TYPE_FULL_DESIGN*/
                        {
                            if false
                            /* TODO: g_Config.m_ClShowQuads || self.map_type == TYPE_FULL_DESIGN*/
                            {
                                if let Some(buffered_map) = pipe.buffered_map {
                                    state.blend_normal();
                                    self.RenderQuadLayer(
                                        buffered_map,
                                        &state,
                                        pipe,
                                        QuadLayerCounter - 1,
                                        &pQLayer.0,
                                        &pQLayer.1,
                                        pGroup,
                                        true,
                                    );
                                } else {
                                    state.blend_normal();
                                    RenderTools::force_render_quads(
                                        pipe,
                                        &state,
                                        pQuads,
                                        pQLayer.0.num_quads as usize,
                                        LayerRenderFlag::Transparent as i32,
                                        |map, game, sys, time_offset_millis, env, channels| {
                                            self.EnvelopeEval(
                                                map,
                                                game,
                                                sys,
                                                time_offset_millis,
                                                env,
                                                channels,
                                            )
                                        },
                                        1.0,
                                    );
                                }
                            }
                        } else {
                            if let Some(buffered_map) = pipe.buffered_map {
                                state.blend_normal();
                                self.RenderQuadLayer(
                                    buffered_map,
                                    &state,
                                    pipe,
                                    QuadLayerCounter - 1,
                                    &pQLayer.0,
                                    &pQLayer.1,
                                    pGroup,
                                    false,
                                );
                            } else {
                                state.blend_normal();
                                RenderTools::render_quads(
                                    pipe,
                                    &state,
                                    pQuads,
                                    pQLayer.0.num_quads as usize,
                                    LayerRenderFlag::Transparent as i32,
                                    |map, game, sys, time_offset_millis, env, channels| {
                                        self.EnvelopeEval(
                                            map,
                                            game,
                                            sys,
                                            time_offset_millis,
                                            env,
                                            channels,
                                        )
                                    },
                                    1.0, // TODO
                                );
                            }
                        }
                    }
                }
                /*else if Render && EntityOverlayVal && IsFrontLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_FRONT));

                       CTile *pFrontTiles = (CTile *)m_pLayers.Map().GetData(pTMap.m_Front);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Front);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderTilemap(pFrontTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque,
                                   EnvelopeEval, this, pTMap.m_ColorEnv, pTMap.m_ColorEnvOffset);
                               Graphics().BlendNormal();
                               RenderTools().RenderTilemap(pFrontTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT,
                                   EnvelopeEval, this, pTMap.m_ColorEnv, pTMap.m_ColorEnvOffset);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsSwitchLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_SWITCH));

                       CSwitchTile *pSwitchTiles = (CSwitchTile *)m_pLayers.Map().GetData(pTMap.m_Switch);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Switch);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CSwitchTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderSwitchmap(pSwitchTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderSwitchmap(pSwitchTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               RenderTools().RenderSwitchOverlay(pSwitchTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal / 100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 3, Color, pTMap, pGroup);
                               if g_Config.m_ClTextEntities)
                               {
                                   Graphics().TextureSet(m_pImages.GetOverlayBottom());
                                   RenderTileLayer(TileLayerCounter - 2, Color, pTMap, pGroup);
                                   Graphics().TextureSet(m_pImages.GetOverlayTop());
                                   RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                               }
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsTeleLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_TELE));

                       CTeleTile *pTeleTiles = (CTeleTile *)m_pLayers.Map().GetData(pTMap.m_Tele);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Tele);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CTeleTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderTelemap(pTeleTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderTelemap(pTeleTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               RenderTools().RenderTeleOverlay(pTeleTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal / 100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 2, Color, pTMap, pGroup);
                               if g_Config.m_ClTextEntities)
                               {
                                   Graphics().TextureSet(m_pImages.GetOverlayCenter());
                                   RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                               }
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsSpeedupLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_SPEEDUP));

                       CSpeedupTile *pSpeedupTiles = (CSpeedupTile *)m_pLayers.Map().GetData(pTMap.m_Speedup);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Speedup);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CSpeedupTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderSpeedupmap(pSpeedupTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderSpeedupmap(pSpeedupTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               RenderTools().RenderSpeedupOverlay(pSpeedupTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal / 100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();

                               // draw arrow -- clamp to the edge of the arrow image
                               Graphics().WrapClamp();
                               Graphics().TextureSet(m_pImages.GetSpeedupArrow());
                               RenderTileLayer(TileLayerCounter - 3, Color, pTMap, pGroup);
                               Graphics().WrapNormal();
                               if g_Config.m_ClTextEntities)
                               {
                                   Graphics().TextureSet(m_pImages.GetOverlayBottom());
                                   RenderTileLayer(TileLayerCounter - 2, Color, pTMap, pGroup);
                                   Graphics().TextureSet(m_pImages.GetOverlayTop());
                                   RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                               }
                           }
                       }
                   }
                   else if Render && EntityOverlayVal && IsTuneLayer)
                   {
                       CMapItemLayerTilemap *pTMap = (CMapItemLayerTilemap *)pLayer;
                       Graphics().TextureSet(m_pImages.GetEntities(MAP_IMAGE_ENTITY_LAYER_TYPE_TUNE));

                       CTuneTile *pTuneTiles = (CTuneTile *)m_pLayers.Map().GetData(pTMap.m_Tune);
                       u32 Size = m_pLayers.Map().GetDataSize(pTMap.m_Tune);

                       if Size >= (size_t)pTMap.m_Width * pTMap.m_Height * sizeof(CTuneTile))
                       {
                           ColorRGBA Color = ColorRGBA(pTMap.m_Color.r() / 255.0, pTMap.m_Color.g() / 255.0, pTMap.m_Color.b() / 255.0, pTMap.m_Color.a() / 255.0 * EntityOverlayVal / 100.0);
                           if !Graphics().IsTileBufferingEnabled())
                           {
                               Graphics().BlendNone();
                               RenderTools().RenderTunemap(pTuneTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LayerRenderFlag::Opaque);
                               Graphics().BlendNormal();
                               RenderTools().RenderTunemap(pTuneTiles, pTMap.m_Width, pTMap.m_Height, 32.0, Color, TileRenderFlag::Extend | LAYERRENDERFLAG_TRANSPARENT);
                               //RenderTools().RenderTuneOverlay(pTuneTiles, pTMap.m_Width, pTMap.m_Height, 32.0, EntityOverlayVal/100.0);
                           }
                           else
                           {
                               Graphics().BlendNormal();
                               RenderTileLayer(TileLayerCounter - 1, Color, pTMap, pGroup);
                           }
                       }
                   }
                */
            }

            /*
            if !g_Config.m_GfxNoclip || self.map_type == TYPE_FULL_DESIGN)
                Graphics().ClipDisable();
             */
        }
        /*
        if !g_Config.m_GfxNoclip || self.map_type == TYPE_FULL_DESIGN)
            Graphics().ClipDisable();*/
    }
}
