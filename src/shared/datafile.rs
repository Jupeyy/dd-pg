use std::{collections::HashMap, ffi::CStr, io::Read, mem::size_of, sync::Arc};

use base::{system::SystemTimeInterface, benchmark};
use flate2::read::ZlibDecoder;
use rayon::{
    prelude::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

use crate::{
    mapdef::{
        read_i32_le, read_u32_le, CEnvPoint, CMapItemEnvelope, CMapItemGroup, CMapItemImage,
        CMapItemInfoSettings, CMapItemLayer, CMapItemLayerQuads, CMapItemLayerSounds,
        CMapItemLayerTilemap, CMapItemSound, CMapItemVersion, CQuad, MapImage, MapItemTypes,
        MapLayer, MapLayerQuad, MapLayerTile, MapLayerTypes, MapTileLayerDetail, ReadFromSlice,
        TilesLayerFlag,
    }, join_all,
};

const DEBUG: i32 = 0;

enum UUIDOffset {
    OFFSET_UUID_TYPE = 0x8000,
}

#[repr(C)]
pub struct CUuid {
    data: [u8; 16],
}

#[repr(C)]
pub struct CItemEx {
    uuid: [i32; std::mem::size_of::<CUuid>() / 4],
}
/*
TODO: impl
static CItemEx FromUuid(CUuid Uuid)
{
    CItemEx Result;
    for(i = 0; i < (int)sizeof(CUuid) / 4: i32, i++)
        Result.m_aUuid[i] = bytes_be_to_int(&Uuid.m_aData[i * 4]);
    return Result;
}

CUuid ToUuid() const
{
    CUuid Result;
    for(i = 0; i < (int)sizeof(CUuid) / 4: i32, i++)
        int_to_bytes_be(&Result.m_aData[i * 4], m_aUuid[i]);
    return Result;
} */

#[derive(Copy, Clone, Default)]
#[repr(C)]
struct CDatafileItemType {
    item_type: i32,
    start: i32,
    num: i32,
}

impl CDatafileItemType {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (item_type, rest) = data.split_at(size_of::<i32>());
        let i_type = read_i32_le(&item_type);

        let (start, rest) = rest.split_at(size_of::<i32>());
        let s = read_i32_le(&start);

        let (num, _rest) = rest.split_at(size_of::<i32>());
        let n = read_i32_le(&num);

        Self {
            item_type: i_type,
            start: s,
            num: n,
        }
    }
}

#[repr(C)]
struct CDatafileItem {
    type_and_id: i32,
    size: i32,
}

impl CDatafileItem {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (type_and_id, rest) = data.split_at(size_of::<i32>());
        let t_and_id = read_i32_le(&type_and_id);

        let (size, _rest) = rest.split_at(size_of::<i32>());
        let s = read_i32_le(&size);

        Self {
            type_and_id: t_and_id,
            size: s,
        }
    }
}

#[repr(C)]
struct CDatafileItemAndData<'a> {
    header: CDatafileItem,
    data: &'a [u8],
}

impl<'a> CDatafileItemAndData<'a> {
    pub fn read_from_slice(data: &'a [u8], item_size: usize) -> Self {
        let header = CDatafileItem::read_from_slice(data);

        let (_, rest) = data.split_at(size_of::<CDatafileItem>());

        let (rest, _) = rest.split_at(item_size);

        Self {
            header: header,
            data: rest,
        }
    }
}

#[derive(Default, Clone, Copy)]
#[repr(C)]
struct CDatafileHeader {
    id: [std::os::raw::c_char; 4],
    version: u32,
    size: u32,
    swap_len: u32,
    num_item_types: u32,
    num_items: u32,
    num_raw_data: u32,
    item_size: u32,
    data_size: u32,
}

impl CDatafileHeader {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let mut rest = data;
        let mut id: [u8; 4] = Default::default();
        id.iter_mut().for_each(|b| {
            let (id, rest2) = rest.split_at(size_of::<std::os::raw::c_char>());
            *b = id[0];
            rest = rest2;
        });

        let (version, rest) = rest.split_at(size_of::<u32>());
        let ver = read_u32_le(version);

        let (size, rest) = rest.split_at(size_of::<u32>());
        let siz = read_u32_le(size);

        let (swaplen, rest) = rest.split_at(size_of::<u32>());
        let swapln = read_u32_le(swaplen);

        let (num_item_types, rest) = rest.split_at(size_of::<u32>());
        let item_types_num = read_u32_le(num_item_types);

        let (num_item, rest) = rest.split_at(size_of::<u32>());
        let item_num = read_u32_le(num_item);

        let (num_raw_data, rest) = rest.split_at(size_of::<u32>());
        let raw_data_num = read_u32_le(num_raw_data);

        let (item_size, rest) = rest.split_at(size_of::<u32>());
        let i_size = read_u32_le(item_size);

        let (data_size, _rest) = rest.split_at(size_of::<u32>());
        let d_size = read_u32_le(data_size);

        Self {
            id: [
                id[0] as std::os::raw::c_char,
                id[1] as std::os::raw::c_char,
                id[2] as std::os::raw::c_char,
                id[3] as std::os::raw::c_char,
            ],
            version: ver,
            size: siz,
            swap_len: swapln,
            num_item_types: item_types_num,
            num_items: item_num,
            num_raw_data: raw_data_num,
            item_size: i_size,
            data_size: d_size,
        }
    }
}

#[derive(Clone, Default)]
#[repr(C)]
struct CDatafileInfo {
    item_types: Vec<CDatafileItemType>,
    item_offsets: Vec<i32>,
    data_offsets: Vec<i32>,
    data_sizes: Vec<i32>,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct CDatafile {
    /*IOHANDLE m_File;
    SHA256_DIGEST m_Sha256;
    unsigned m_Crc;*/
    info: CDatafileInfo,
    header: CDatafileHeader,
}

pub enum ReadFile {
    // contains the image index
    Image(usize, Vec<u8>),
}

pub struct CDatafileWrapper {
    pub data_file: CDatafile,
    pub name: String,

    versions: Vec<CMapItemVersion>,
    infos: Vec<CMapItemInfoSettings>,
    pub images: Vec<MapImage>,
    envelopes: Vec<CMapItemEnvelope>,
    groups: Vec<CMapItemGroup>,
    pub layers: Vec<MapLayer>,
    env_points: Vec<Vec<CEnvPoint>>,
    sounds: Vec<CMapItemSound>,

    game_layer_index: usize,
    game_group_index: usize,
    //m_pGameGroupEx: *mut CMapItemGroupEx,
    tele_layer_index: usize,
    speed_layer_index: usize,
    front_layer_index: usize,
    switch_layer_index: usize,
    tune_layer_index: usize,

    // files to read, if the user of this object
    // wants to have support for images etc.
    pub read_files: HashMap<String, ReadFile>,
}

#[derive(Default)]
pub struct MapFileOpenOptions {
    pub do_benchmark: bool,
    pub dont_load_map_item: [bool; MapItemTypes::MAPITEMTYPE_COUNT as usize],
}

#[derive(Default)]
pub struct MapFileLayersReadOptions {
    pub do_benchmark: bool,
    pub dont_load_design_layers: bool,
}

#[derive(Default)]
pub struct MapFileImageReadOptions {
    pub do_benchmark: bool,
}

impl CDatafileWrapper {
    pub fn new() -> CDatafileWrapper {
        CDatafileWrapper {
            data_file: Default::default(),
            name: String::new(),
            versions: Vec::new(),
            infos: Vec::new(),
            images: Vec::new(),
            envelopes: Vec::new(),
            groups: Vec::new(),
            layers: Vec::new(),
            env_points: Vec::new(),
            sounds: Vec::new(),

            game_layer_index: usize::MAX,
            game_group_index: usize::MAX,
            //m_pGameGroupEx: std::ptr::null_mut(),
            tele_layer_index: usize::MAX,
            speed_layer_index: usize::MAX,
            front_layer_index: usize::MAX,
            switch_layer_index: usize::MAX,
            tune_layer_index: usize::MAX,

            read_files: HashMap::default(),
        }
    }

    /**
     * Returns a tuple of various information about the file:
     * - the a slice of the data containers of the file vec
     *
     */
    pub fn Open<'a>(
        &mut self,
        data_param: &'a Vec<u8>,
        file_name: &str,
        thread_pool: &rayon::ThreadPool,
        options: &MapFileOpenOptions,
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
    ) -> anyhow::Result<&'a [u8]> {
        let do_benchmark = options.do_benchmark;
        self.name = file_name.to_string();
        //log_trace("datafile", "loading. filename='%s'", pFilename);

        // take the CRC of the file and store it
        /*unsigned Crc = 0;
        SHA256_DIGEST Sha256;
        {
            enum
            {
                BUFFER_SIZE = 64 * 1024
            };

            SHA256_CTX Sha256Ctxt;
            sha256_init(&Sha256Ctxt);
            unsigned char aBuffer[BUFFER_SIZE];

            while(true)
            {
                unsigned Bytes = io_read(File, aBuffer, BUFFER_SIZE);
                if(Bytes == 0)
                    break;
                Crc = crc32(Crc, aBuffer, Bytes);
                sha256_update(&Sha256Ctxt, aBuffer, Bytes);
            }
            Sha256 = sha256_finish(&Sha256Ctxt);

            io_seek(File, 0, IOSEEK_START);
        }*/
        let mut data_file: CDatafile = CDatafile::default();
        let mut read_data = data_param.as_slice();

        let mut items: Vec<CDatafileItemAndData> = Vec::new();
        let mut data_start: &[u8] = &[];

        if !benchmark!(
            do_benchmark,
            &sys,
            "\tloading the map header, items and data",
            || {
                // TODO: change this header
                let header_size = std::mem::size_of::<CDatafileHeader>();
                if header_size <= read_data.len() {
                    data_file.header = CDatafileHeader::read_from_slice(read_data);
                    (_, read_data) = read_data.split_at(header_size);
                } else {
                    return false;
                }
                if data_file.header.id[0] != 'A' as i8
                    || data_file.header.id[1] != 'T' as i8
                    || data_file.header.id[2] != 'A' as i8
                    || data_file.header.id[3] != 'D' as i8
                {
                    if data_file.header.id[0] != 'D' as i8
                        || data_file.header.id[1] != 'A' as i8
                        || data_file.header.id[2] != 'T' as i8
                        || data_file.header.id[3] != 'A' as i8
                    {
                        /*dbg_msg(
                            "datafile",
                            "wrong signature. %x %x %x %x",
                            Header.m_aID[0],
                            Header.m_aID[1],
                            Header.m_aID[2],
                            Header.m_aID[3],
                        );*/
                        return false;
                    }
                }

                // data_file.m_Header.m_Version != 3 &&
                if data_file.header.version != 4 {
                    // TODO dbg_msg("datafile", "wrong version. version=%x", Header.m_Version);
                    return false;
                }

                // read in the rest except the data
                let mut Size: u32 = 0;
                Size += data_file.header.num_item_types
                    * std::mem::size_of::<CDatafileItemType>() as u32;
                Size += (data_file.header.num_items + data_file.header.num_raw_data)
                    * std::mem::size_of::<u32>() as u32;
                if data_file.header.version == 4 {
                    Size += data_file.header.num_raw_data * std::mem::size_of::<u32>() as u32;
                    // v4 has uncompressed data sizes as well
                }
                Size += data_file.header.item_size;

                /*TODO:(*pTmpDataFile).m_File = File;
                (*pTmpDataFile).m_Sha256 = Sha256;
                (*pTmpDataFile).m_Crc = Crc;*/

                if read_data.len() < (Size as usize) {
                    //TODO dbg_msg("datafile", "couldn't load the whole thing, wanted=%d got=%d", Size, ReadSize);
                    return false;
                }

                /*
                if(DEBUG)
                {
                    dbg_msg("datafile", "allocsize=%d", AllocSize);
                    dbg_msg("datafile", "readsize=%d", ReadSize);
                    dbg_msg("datafile", "swaplen=%d", Header.m_Swaplen);
                    dbg_msg("datafile", "item_size=%d", (*self.m_pDataFile).m_Header.m_ItemSize);
                } */

                let size_of_item = size_of::<CDatafileItemType>();
                for _i in 0..data_file.header.num_item_types {
                    data_file
                        .info
                        .item_types
                        .push(CDatafileItemType::read_from_slice(read_data));
                    (_, read_data) = read_data.split_at(size_of_item);
                }

                let size_of_item = size_of::<i32>();
                for _i in 0..data_file.header.num_items {
                    data_file.info.item_offsets.push(read_i32_le(read_data));
                    (_, read_data) = read_data.split_at(size_of_item);
                }

                let size_of_item = size_of::<i32>();
                for _i in 0..data_file.header.num_raw_data {
                    data_file.info.data_offsets.push(read_i32_le(read_data));
                    (_, read_data) = read_data.split_at(size_of_item);
                }

                let size_of_item = size_of::<i32>();
                for _i in 0..data_file.header.num_raw_data {
                    data_file.info.data_sizes.push(read_i32_le(read_data));
                    (_, read_data) = read_data.split_at(size_of_item);
                }

                if data_file.header.version == 4 {
                    let (itemsstart, rest) =
                        read_data.split_at(data_file.header.item_size as usize);
                    read_data = rest;

                    for i in 0..data_file.header.num_items as usize {
                        let offset = data_file.info.item_offsets[i] as usize;
                        let (_, item_data) = itemsstart.split_at(offset);

                        items.push(CDatafileItemAndData::read_from_slice(
                            item_data,
                            Self::GetItemSize(&data_file.header, &data_file.info, i as i32)
                                as usize,
                        ));
                    }
                } else {
                    panic!("not supported");
                }

                let (datas, rest) = read_data.split_at(data_file.header.data_size as usize);
                data_start = datas;
                read_data = rest;

                return true;
            },
        ) {
            return Err(anyhow::Error::msg("File could not be opened"));
        }

        // read items
        thread_pool.install(|| {
            join_all!(
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_VERSION as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map version", || {
                            // MAPITEMTYPE_VERSION
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CMapItemVersion>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_VERSION as i32,
                                &mut start,
                                &mut num,
                            );
                            for i in 0..num as usize {
                                let data = &items[start as usize + i].data[0..item_size];
                                self.versions.push(CMapItemVersion::read_from_slice(data))
                            }
                        });
                    }
                },
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_INFO as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map info", || {
                            // MAPITEMTYPE_INFO
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CMapItemInfoSettings>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_INFO as i32,
                                &mut start,
                                &mut num,
                            );
                            for i in 0..num as usize {
                                let data = &items[start as usize + i].data[0..item_size];
                                self.infos.push(CMapItemInfoSettings::read_from_slice(data))
                            }
                        });
                    }
                },
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_IMAGE as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map images", || {
                            //MAPITEMTYPE_IMAGE
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CMapItemImage>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_IMAGE as i32,
                                &mut start,
                                &mut num,
                            );
                            self.images
                                .resize_with(num as usize, || MapImage::default());
                            self.images.par_iter_mut().enumerate().for_each(|(i, img)| {
                                let data = &items[start as usize + i].data[0..item_size];
                                img.item_data = CMapItemImage::read_from_slice(data);

                                // read the image name
                                let data_name = Self::uncompress_data(
                                    &data_file,
                                    img.item_data.image_name as usize,
                                    data_start,
                                );
                                let name_cstr =
                                    CStr::from_bytes_with_nul(data_name.as_slice()).unwrap();
                                img.img_name = name_cstr.to_str().unwrap().to_string();
                            });
                            self.images.iter().enumerate().for_each(|(index, img)| {
                                if img.item_data.external != 0 {
                                    // add the external image to the read files
                                    self.read_files.insert(
                                        "mapres/".to_string() + &img.img_name + ".png",
                                        ReadFile::Image(index, Vec::new()),
                                    );
                                }
                            });
                        });
                    }
                },
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_ENVELOPE as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map envelopes", || {
                            //MAPITEMTYPE_ENVELOPE
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CMapItemEnvelope>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_ENVELOPE as i32,
                                &mut start,
                                &mut num,
                            );
                            for i in 0..num as usize {
                                let data = &items[start as usize + i].data[0..item_size];
                                self.envelopes.push(CMapItemEnvelope::read_from_slice(data))
                            }
                        });
                    }
                },
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_GROUP as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map groups", || {
                            //MAPITEMTYPE_GROUP
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CMapItemGroup>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_GROUP as i32,
                                &mut start,
                                &mut num,
                            );
                            for i in 0..num as usize {
                                let data = &items[start as usize + i].data[0..item_size];
                                self.groups.push(CMapItemGroup::read_from_slice(data))
                            }
                        });
                    }
                },
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_LAYER as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map layers", || {
                            //MAPITEMTYPE_LAYER
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CMapItemLayer>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_LAYER as i32,
                                &mut start,
                                &mut num,
                            );
                            self.layers = vec![MapLayer::Unknown(Default::default()); num as usize];
                            self.layers
                                .par_iter_mut()
                                .enumerate()
                                .for_each(|(i, map_layer)| {
                                    let data = &items[start as usize + i].data[0..item_size];
                                    let layer = CMapItemLayer::read_from_slice(data);

                                    if layer.item_layer == MapLayerTypes::LAYERTYPE_TILES as i32 {
                                        let item_size = size_of::<CMapItemLayerTilemap>();
                                        let data = &items[start as usize + i].data[0..item_size];
                                        let tile_layer =
                                            CMapItemLayerTilemap::read_from_slice(data);

                                        let tile_layer_impl = MapTileLayerDetail::Tile();
                                        *map_layer = MapLayer::Tile(MapLayerTile(
                                            tile_layer,
                                            tile_layer_impl,
                                            Vec::new(),
                                        ));
                                    } else if layer.item_layer
                                        == MapLayerTypes::LAYERTYPE_QUADS as i32
                                    {
                                        let item_size = size_of::<CMapItemLayerQuads>();
                                        let data = &items[start as usize + i].data[0..item_size];
                                        let quad_layer_info =
                                            CMapItemLayerQuads::read_from_slice(data);

                                        *map_layer = MapLayer::Quads(MapLayerQuad(
                                            quad_layer_info,
                                            Vec::new(),
                                        ));
                                    } else if layer.item_layer
                                        == MapLayerTypes::LAYERTYPE_SOUNDS as i32
                                    {
                                        let item_size = size_of::<CMapItemLayerSounds>();
                                        let data = &items[start as usize + i].data[0..item_size];
                                        *map_layer = MapLayer::Sound(
                                            CMapItemLayerSounds::read_from_slice(data),
                                        );
                                    } else {
                                        *map_layer = MapLayer::Unknown(layer);
                                    }
                                });
                        });
                    }
                },
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_ENVPOINTS as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map env-points", || {
                            //MAPITEMTYPE_ENVPOINTS
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CEnvPoint>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_ENVPOINTS as i32,
                                &mut start,
                                &mut num,
                            );
                            for i in 0..num as usize {
                                let item_count = items[start as usize + i].data.len() / item_size;
                                let mut env_points: Vec<CEnvPoint> = Vec::new();
                                for _n in 0..item_count {
                                    let data = &items[start as usize + i].data[0..item_size];
                                    env_points.push(CEnvPoint::read_from_slice(data));
                                }
                                self.env_points.push(env_points);
                            }
                        });
                    }
                },
                || {
                    if !options.dont_load_map_item[MapItemTypes::MAPITEMTYPE_SOUND as usize] {
                        benchmark!(do_benchmark, &sys, "\tloading the map sounds", || {
                            //MAPITEMTYPE_SOUND
                            let mut start = i32::default();
                            let mut num = i32::default();
                            let item_size = size_of::<CMapItemSound>();
                            Self::GetType(
                                &data_file,
                                MapItemTypes::MAPITEMTYPE_SOUND as i32,
                                &mut start,
                                &mut num,
                            );
                            for i in 0..num as usize {
                                let data = &items[start as usize + i].data[0..item_size];
                                self.sounds.push(CMapItemSound::read_from_slice(data))
                            }
                        });
                    }
                }
            );
        });

        self.data_file = data_file; //pTmpDataFile;

        return Ok(data_start);
    }

    pub fn read_map_layers(
        data_file: &CDatafile,
        layers: &mut Vec<MapLayer>,
        data_start: &[u8],
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
        options: &MapFileLayersReadOptions,
    ) {
        benchmark!(
            options.do_benchmark,
            &sys,
            "\tloading the map layers tiles",
            || {
                layers
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(_i, map_layer)| {
                        if let MapLayer::Tile(tile_layer) = map_layer {
                            let _item_size = size_of::<CMapItemLayerTilemap>();
                            let tiles_data_index = tile_layer.0.data;

                            let mut is_entity_layer = false;

                            if (tile_layer.0.flags & TilesLayerFlag::TILESLAYERFLAG_GAME as i32)
                                != 0
                            {
                                is_entity_layer = true;
                            }

                            let mut tile_layer_impl = MapTileLayerDetail::Tile();
                            if (tile_layer.0.flags & TilesLayerFlag::TILESLAYERFLAG_TELE as i32)
                                != 0
                            {
                                tile_layer_impl = MapTileLayerDetail::Tele(Self::read_tiles(
                                    data_file,
                                    tile_layer.0.tele,
                                    tile_layer.0.width as usize,
                                    tile_layer.0.height as usize,
                                    data_start,
                                ));
                                is_entity_layer = true;
                            } else if (tile_layer.0.flags
                                & TilesLayerFlag::TILESLAYERFLAG_SPEEDUP as i32)
                                != 0
                            {
                                tile_layer_impl = MapTileLayerDetail::Speedup(Self::read_tiles(
                                    data_file,
                                    tile_layer.0.speedup,
                                    tile_layer.0.width as usize,
                                    tile_layer.0.height as usize,
                                    data_start,
                                ));
                                is_entity_layer = true;
                            } else if (tile_layer.0.flags
                                & TilesLayerFlag::TILESLAYERFLAG_SWITCH as i32)
                                != 0
                            {
                                tile_layer_impl = MapTileLayerDetail::Switch(Self::read_tiles(
                                    data_file,
                                    tile_layer.0.switch,
                                    tile_layer.0.width as usize,
                                    tile_layer.0.height as usize,
                                    data_start,
                                ));
                                is_entity_layer = true;
                            } else if (tile_layer.0.flags
                                & TilesLayerFlag::TILESLAYERFLAG_TUNE as i32)
                                != 0
                            {
                                tile_layer_impl = MapTileLayerDetail::Tune(Self::read_tiles(
                                    data_file,
                                    tile_layer.0.tune,
                                    tile_layer.0.width as usize,
                                    tile_layer.0.height as usize,
                                    data_start,
                                ));
                                is_entity_layer = true;
                            }

                            let tiles = if is_entity_layer || !options.dont_load_design_layers {
                                Self::read_tiles(
                                    data_file,
                                    tiles_data_index,
                                    tile_layer.0.width as usize,
                                    tile_layer.0.height as usize,
                                    data_start,
                                )
                            } else {
                                Vec::new()
                            };

                            *map_layer = MapLayer::Tile(MapLayerTile(
                                tile_layer.0.clone(),
                                tile_layer_impl,
                                tiles,
                            ));
                        } else if let MapLayer::Quads(quad_layer) = map_layer {
                            let quads = Self::read_quads(
                                data_file,
                                quad_layer.0.data,
                                quad_layer.0.num_quads as usize,
                                data_start,
                            );
                            *map_layer = MapLayer::Quads(MapLayerQuad(quad_layer.0.clone(), quads));
                        } else if let MapLayer::Sound(_sound_layer) = map_layer {
                            // TODO: nothing to do yet, but actually sounds are loaded
                        }
                    });
            }
        );
    }

    pub fn read_image_data(
        data_file: &CDatafile,
        images: &Vec<MapImage>,
        data_start: &[u8],
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
        options: &MapFileImageReadOptions,
    ) -> Vec<Option<(u32, u32, Vec<u8>)>> {
        let mut res: Vec<Option<(u32, u32, Vec<u8>)>> = Vec::new();
        res.resize(images.len(), Default::default());
        benchmark!(
            options.do_benchmark,
            &sys,
            "\tloading the map internal images",
            || {
                res.par_iter_mut().enumerate().for_each(|(i, img)| {
                    let img_data = &images[i];
                    if img_data.item_data.external == 0 {
                        // read the image data
                        *img = Some((
                            img_data.item_data.width as u32,
                            img_data.item_data.height as u32,
                            Self::uncompress_data(
                                data_file,
                                img_data.item_data.image_data as usize,
                                data_start,
                            ),
                        ));
                    }
                });
            }
        );
        res
    }

    fn read_tiles<T>(
        data_file: &CDatafile,
        data_index: i32,
        width: usize,
        height: usize,
        data_start: &[u8],
    ) -> Vec<T>
    where
        T: ReadFromSlice + Default + Clone + Send + Sync,
    {
        if data_index != -1 {
            let tile_size = size_of::<T>();
            let uncompressed_data =
                Self::uncompress_data(data_file, data_index as usize, data_start);
            let tiles_sliced = uncompressed_data.as_slice();
            let mut tiles = vec![Default::default(); width * height];
            tiles
                .par_chunks_exact_mut(width)
                .enumerate()
                .for_each(|(y, tiles_width)| {
                    for x in 0..width {
                        let tile_index = y * width + x;
                        let tile_sliced = &tiles_sliced
                            [(tile_index * tile_size)..(tile_index * tile_size) + tile_size];

                        tiles_width[x] = T::read_from_slice(tile_sliced);
                    }
                });
            return tiles;
        }
        Vec::new()
    }

    fn read_quads(
        data_file: &CDatafile,
        data_index: i32,
        num_quads: usize,
        data_start: &[u8],
    ) -> Vec<CQuad> {
        if data_index != -1 {
            let quad_size = size_of::<CQuad>();
            let uncompressed_data =
                Self::uncompress_data(data_file, data_index as usize, data_start);
            let quads_sliced = uncompressed_data.as_slice();
            let mut quads = vec![Default::default(); num_quads];
            quads.par_iter_mut().enumerate().for_each(|(index, quad)| {
                let quad_sliced = &quads_sliced[index * quad_size..(index * quad_size) + quad_size];
                *quad = CQuad::read_from_slice(quad_sliced);
            });
            return quads;
        }
        Vec::new()
    }

    fn uncompress_data(data_file: &CDatafile, index: usize, data_start: &[u8]) -> Vec<u8> {
        // v4 has compressed data
        let UncompressedSize = data_file.info.data_sizes[index];

        // read the compressed data
        let data_split = Self::get_data_slice(&data_file, index, data_start);
        let pTemp = data_split;

        // decompress the data, TODO: check for errors
        let mut d = ZlibDecoder::new(pTemp);

        let mut data = Vec::<u8>::new();
        data.reserve(UncompressedSize as usize);
        d.read_to_end(&mut data).unwrap();
        data
    }

    fn GetInternalItemType(ExternalType: i32) -> i32 {
        if ExternalType < UUIDOffset::OFFSET_UUID_TYPE as i32 {
            return ExternalType;
        }
        /* TODO! CUuid Uuid = g_UuidManager.GetUuid(ExternalType);
        Start, Num: i32,
        GetType(ITEMTYPE_EX, &Start, &Num);
        for(i = Start; i < Start + Num: i32, i++)
        {
            if(GetItemSize(i) < (int)sizeof(CItemEx))
            {
                continue;
            }
            ID: i32,
            if(Uuid == ((const CItemEx *)GetItem(i, 0, &ID))->ToUuid())
            {
                return ID;
            }
        }*/
        return -1;
    }

    fn GetType(data_file: &CDatafile, Type: i32, pStart: &mut i32, pNum: &mut i32) {
        *pStart = 0;
        *pNum = 0;

        let real_type = Self::GetInternalItemType(Type);
        for i in 0..data_file.header.num_item_types as usize {
            if data_file.info.item_types[i].item_type == real_type {
                *pStart = data_file.info.item_types[i].start;
                *pNum = data_file.info.item_types[i].num;
                return;
            }
        }
    }

    pub fn NumGroups(&self) -> i32 {
        self.groups.len() as i32
    }

    fn GetItemSize(header: &CDatafileHeader, info: &CDatafileInfo, Index: i32) -> i32 {
        if Index == header.num_items as i32 - 1 {
            return header.item_size as i32
                - info.item_offsets[Index as usize]
                - std::mem::size_of::<CDatafileItem>() as i32;
        }
        return info.item_offsets[Index as usize + 1] as i32
            - info.item_offsets[Index as usize]
            - std::mem::size_of::<CDatafileItem>() as i32;
    }

    fn get_data_slice<'a>(data_file: &CDatafile, index: usize, data_start: &'a [u8]) -> &'a [u8] {
        let data_start_off = data_file.info.data_offsets[index as usize] as usize;
        let (_, offset_data) = data_start.split_at(data_start_off);
        let (data_split, _) =
            offset_data
                .split_at(Self::GetDataSize(&data_file.header, &data_file.info, index) as usize);
        data_split
    }

    fn GetDataSize(header: &CDatafileHeader, info: &CDatafileInfo, Index: usize) -> i32 {
        if Index as i32 == header.num_raw_data as i32 - 1 {
            return header.data_size as i32 - info.data_offsets[Index as usize];
        }
        return info.data_offsets[Index as usize + 1] - info.data_offsets[Index as usize];
    }

    fn InitTilemapSkip(&mut self, thread_pool: &rayon::ThreadPool) {
        for g in 0..self.NumGroups() as usize {
            let pGroup = &self.groups[g];
            for l in 0..pGroup.num_layers as usize {
                let pLayer = &mut self.layers[pGroup.start_layer as usize + l];

                if let MapLayer::Tile(MapLayerTile(tile_layer, _, tiles)) = pLayer {
                    let pTiles = tiles;
                    thread_pool.install(|| {
                        pTiles
                            .par_chunks_mut(tile_layer.width as usize)
                            .for_each(|pTilesChunk| {
                                let mut x = 0;
                                while x < tile_layer.width {
                                    let mut SkippedX: i32 = 1;
                                    while x + SkippedX < tile_layer.width && SkippedX < 255 {
                                        if pTilesChunk[x as usize + SkippedX as usize].index > 0 {
                                            break;
                                        }

                                        SkippedX += 1;
                                    }

                                    pTilesChunk[x as usize].skip = (SkippedX - 1) as u8;
                                    x += SkippedX;
                                }
                            });
                    });
                }
            }
        }
    }

    pub fn InitLayers(&mut self, thread_pool: &rayon::ThreadPool) {
        for g in 0..self.NumGroups() as usize {
            let pGroup = &mut self.groups[g];
            //let pGroupEx = self.GetGroupExUnsafe(g);
            for l in 0..pGroup.num_layers as usize {
                let layer_index = pGroup.start_layer as usize + l;
                let pLayer = &mut self.layers[layer_index];

                if let MapLayer::Tile(MapLayerTile(tile_layer, _, _)) = pLayer {
                    if (tile_layer.flags & TilesLayerFlag::TILESLAYERFLAG_GAME as i32) != 0 {
                        self.game_layer_index = layer_index;
                        self.game_group_index = g;
                        //self.m_pGameGroupEx = pGroupEx;

                        // make sure the game group has standard settings
                        pGroup.offset_x = 0;
                        pGroup.offset_y = 0;
                        pGroup.parallax_x = 100;
                        pGroup.parallax_y = 100;

                        if pGroup.version >= 2 {
                            pGroup.use_clipping = 0;
                            pGroup.clip_x = 0;
                            pGroup.clip_y = 0;
                            pGroup.clip_w = 0;
                            pGroup.clip_h = 0;
                        }

                        /*if !pGroupEx.is_null() {
                            (*pGroupEx).m_ParallaxZoom = 100;
                        }*/

                        //break;
                    }
                    if (tile_layer.flags & TilesLayerFlag::TILESLAYERFLAG_TELE as i32) != 0 {
                        self.tele_layer_index = layer_index;
                    }
                    if (tile_layer.flags & TilesLayerFlag::TILESLAYERFLAG_SPEEDUP as i32) != 0 {
                        self.speed_layer_index = layer_index;
                    }
                    if (tile_layer.flags & TilesLayerFlag::TILESLAYERFLAG_FRONT as i32) != 0 {
                        self.front_layer_index = layer_index;
                    }
                    if (tile_layer.flags & TilesLayerFlag::TILESLAYERFLAG_SWITCH as i32) != 0 {
                        self.switch_layer_index = layer_index;
                    }
                    if (tile_layer.flags & TilesLayerFlag::TILESLAYERFLAG_TUNE as i32) != 0 {
                        self.tune_layer_index = layer_index;
                    }
                }
            }
        }

        self.InitTilemapSkip(thread_pool);
    }

    pub fn is_game_layer(&self, layer_index: usize) -> bool {
        self.game_layer_index == layer_index
    }

    pub fn is_tele_layer(&self, layer_index: usize) -> bool {
        self.tele_layer_index == layer_index
    }

    pub fn is_speedup_layer(&self, layer_index: usize) -> bool {
        self.speed_layer_index == layer_index
    }

    pub fn is_front_layer(&self, layer_index: usize) -> bool {
        self.front_layer_index == layer_index
    }

    pub fn is_switch_layer(&self, layer_index: usize) -> bool {
        self.switch_layer_index == layer_index
    }

    pub fn is_tune_layer(&self, layer_index: usize) -> bool {
        self.tune_layer_index == layer_index
    }

    pub fn get_game_layer(&self) -> &MapLayerTile {
        let layer = &self.layers[self.game_layer_index];
        if let MapLayer::Tile(layer) = layer {
            return layer;
        }
        panic!("layer does not exists");
    }

    pub fn get_game_group(&self) -> &CMapItemGroup {
        self.get_group(self.game_group_index)
    }

    pub fn get_layer(&self, index: usize) -> &MapLayer {
        &self.layers[index]
    }

    pub fn get_group(&self, index: usize) -> &CMapItemGroup {
        &self.groups[index]
    }

    pub fn env_count(&self) -> usize {
        self.envelopes.len()
    }

    pub fn get_env(&self, index: usize) -> &CMapItemEnvelope {
        &self.envelopes[index]
    }

    pub fn env_point_count(&self) -> usize {
        self.env_points.len()
    }

    pub fn get_env_points(&self) -> &[Vec<CEnvPoint>] {
        self.env_points.as_slice()
    }
}
