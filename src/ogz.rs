use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use byteorder::ReadBytesExt;
use byteorder::LittleEndian;
use flate2;

#[derive(Debug)]
enum Engine {
	Sauerbraten,
	Tesseract
}

#[derive(Debug)]
enum Var {
	IVar(u32),
	FVar(f32),
	SVar(String)
}

#[derive(Debug, Default, Clone)]
struct VSlot {
	changed: u32,
	params: Vec<(Vec<u8>, [f32; 4])>,
	scale: f32,
	rotation: u32,
	offset: [u32; 2],
	scroll: [f32; 2],
	layer: u32,
	alphaback: f32,
	alphafront: f32,
	colorscale: [f32; 3]
}

type Cubes = [Cube; 8];

#[derive(Debug)]
struct Cube {
	children: Option<Box<Cubes>>,
	// ext: Option<Box<CubeExt>>,
	edges: [u8; 12],
	texture: [u16; 6],
	material: u16,
	merged: u8,
	escaped_visible: u8
}

struct SurfaceInfo {
	lmid: [u8; 2],
	verts: u8,
	numverts: u8
}

const DEFAULT_GEOM: u16 = 1;

impl Cube {
	fn setface(&mut self, face: [u8; 4]) {
		self.edges[0] = face[0];
		self.edges[1] = face[1];
		self.edges[2] = face[2];
		self.edges[3] = face[3];
		self.edges[4] = face[0];
		self.edges[5] = face[1];
		self.edges[6] = face[2];
		self.edges[7] = face[3];
		self.edges[8] = face[0];
		self.edges[9] = face[1];
		self.edges[10] = face[2];
		self.edges[11] = face[3];
	}
	fn solidfaces(&mut self) {
		self.setface([0x80, 0x80, 0x80, 0x80])
	}
	fn emptyfaces(&mut self) {
		self.setface([0x00, 0x00, 0x00, 0x00])
	}
}

impl Default for Cube {
	fn default() -> Self { Cube {
		children: None,
		edges: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
		texture: [DEFAULT_GEOM; 6],
		material: 0,
		merged: 0,
		escaped_visible: 0
	} }
}

#[allow(unused_variables, non_snake_case)]
fn read_vslot(rdr: &mut std::io::Cursor<Vec<u8>>, vs: &mut VSlot, changed: u32) {

	let (VSLOT_SHPARAM, VSLOT_SCALE, VSLOT_ROTATION, VSLOT_OFFSET,
		VSLOT_SCROLL, VSLOT_LAYER, VSLOT_ALPHA, VSLOT_COLOR, VSLOT_NUM) = (0,1,2,3,4,5,6,7,8);

	vs.changed = changed;
	if vs.changed & (1<<VSLOT_SHPARAM) != 0 {
		let numparams = rdr.read_u16::<LittleEndian>().unwrap();
		vs.params.clear();
		for _i in 0..numparams {
			let nlen = rdr.read_u16::<LittleEndian>().unwrap();

			let mut buf: Vec<u8> = Vec::new();
			buf.resize(nlen as usize, 0u8);
			rdr.read(&mut buf[..]).unwrap();
			let name = buf;

			let mut val: [f32; 4] = [0.0f32; 4];
			val[0] = rdr.read_f32::<LittleEndian>().unwrap();
			val[1] = rdr.read_f32::<LittleEndian>().unwrap();
			val[2] = rdr.read_f32::<LittleEndian>().unwrap();
			val[3] = rdr.read_f32::<LittleEndian>().unwrap();
			vs.params.push((name, val))
		}
	}

	if vs.changed & (1<<VSLOT_SCALE) != 0 {
		vs.scale = rdr.read_f32::<LittleEndian>().unwrap()
	}

	if vs.changed & (1<<VSLOT_ROTATION) != 0 {
		vs.rotation = rdr.read_u32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_OFFSET) != 0 {
		vs.offset[0] = rdr.read_u32::<LittleEndian>().unwrap();
		vs.offset[1] = rdr.read_u32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_SCROLL) != 0 {
		vs.scroll[0] = rdr.read_f32::<LittleEndian>().unwrap();
		vs.scroll[1] = rdr.read_f32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_LAYER) != 0 {
		vs.layer = rdr.read_u32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_ALPHA) != 0 {
		vs.alphafront = rdr.read_f32::<LittleEndian>().unwrap();
		vs.alphaback = rdr.read_f32::<LittleEndian>().unwrap();
	}

	if vs.changed & (1<<VSLOT_COLOR) != 0 {
		vs.colorscale[0] = rdr.read_f32::<LittleEndian>().unwrap();
		vs.colorscale[1] = rdr.read_f32::<LittleEndian>().unwrap();
		vs.colorscale[2] = rdr.read_f32::<LittleEndian>().unwrap();
	}
}

#[allow(unused_variables, unused_parens)]
fn read_cube(rdr: &mut std::io::Cursor<Vec<u8>>, c: &mut Cube, x: i32, y: i32, z: i32, s: i32) -> std::io::Result<()> {

	assert!(s > 0);

	let mapversion = 33;

	let mut haschildren = false;
	let octsav = rdr.read_u8()?;
	println!("{} {} {} + {}: {:x}", x, y, z, s, octsav);
	match (octsav & 7) {
		0 /* OCTSAV_CHILDREN */ => {
			return read_children(rdr, x, y, z, s >> 1)
		}
		1 /* OCTSAV_EMPTY */ => c.emptyfaces(),
		2 /* OCTSAV_SOLID */ => c.solidfaces(),
		3 /* OCTSAV_NORMAL */ => rdr.read_exact(&mut c.edges[..])?,
		4 /* OCTSAV_LODCUBE */ => haschildren = true,
		_ => panic!()
	}

	for ct in &mut c.texture { *ct = rdr.read_u16::<LittleEndian>()?; }
	println!("{:?}", c.texture);

	if mapversion < 7 {
		panic!()
	} else if mapversion <= 31 {
		panic!()
	} else {
		if (octsav & 0x40) != 0 {
			if mapversion <= 32 {
				panic!()
			} else {
				c.material = rdr.read_u16::<LittleEndian>()?;
			}
		}
		if (octsav & 0x80) != 0 {
			c.merged = rdr.read_u8()?;
		}
		if (octsav & 0x20) != 0 {
			let surfmask = rdr.read_u8()?;
			let totalverts = rdr.read_u8()?;
			println!("  {:x}", surfmask);
			for i in 0..6 { if surfmask & (1<<i) != 0 {
				let lmid = rdr.read_u16::<LittleEndian>()?;
				let vertmask = rdr.read_u8()?;
				let numverts = rdr.read_u8()?;
				let layerverts = numverts & 15;
				let mut hasxyz = vertmask & 0x04 != 0;
				let mut hasuv = vertmask & 0x40 != 0;
				let mut hasnorm = vertmask & 0x80 != 0;
				println!("  {} {:x} {:x}", i, vertmask, numverts);
				if numverts == 0 { continue }
				if layerverts == 4 {
					if hasxyz && (vertmask & 1) != 0 {
						rdr.read_u16::<LittleEndian>()?; // TODO
						rdr.read_u16::<LittleEndian>()?; // TODO
						rdr.read_u16::<LittleEndian>()?; // TODO
						rdr.read_u16::<LittleEndian>()?; // TODO
						hasxyz = false;
					}
					if hasuv && (vertmask & 2) != 0 {
						rdr.read_u16::<LittleEndian>()?; // TODO
						rdr.read_u16::<LittleEndian>()?; // TODO
						rdr.read_u16::<LittleEndian>()?; // TODO
						rdr.read_u16::<LittleEndian>()?; // TODO

						if (numverts & 0x80) != 0 {
							rdr.read_u16::<LittleEndian>()?; // TODO
							rdr.read_u16::<LittleEndian>()?; // TODO
							rdr.read_u16::<LittleEndian>()?; // TODO
							rdr.read_u16::<LittleEndian>()?; // TODO
						}

						hasuv = false;
					}
				}
				if hasnorm && (vertmask & 8) != 0 {
					rdr.read_u16::<LittleEndian>()?; // TODO
					hasnorm = false;
				}
				if hasxyz || hasuv || hasnorm {
					for k in 0..layerverts {
						if hasxyz {
							rdr.read_u16::<LittleEndian>()?; // TODO
							rdr.read_u16::<LittleEndian>()?; // TODO
						}
						if hasuv {
							rdr.read_u16::<LittleEndian>()?; // TODO
							rdr.read_u16::<LittleEndian>()?; // TODO
						}
						if hasnorm {
							rdr.read_u16::<LittleEndian>()?; // TODO
						}
					}
				}
				if (numverts & 0x80 /* LAYER_DUP */) != 0 {
					for k in 0..layerverts {
						if hasuv {
							rdr.read_u16::<LittleEndian>()?; // TODO
							rdr.read_u16::<LittleEndian>()?; // TODO
						}
					}
				}
			} }
		}
	}

	if haschildren {
		read_children(rdr, x, y, z, s >> 1)
	} else {
		Ok(())
	}
}

fn read_children(rdr: &mut std::io::Cursor<Vec<u8>>, x: i32, y: i32, z: i32, s: i32) -> std::io::Result<()> {
	let mut cubes = Box::new(Cubes::default());
	read_cube(rdr, &mut cubes[0], x,   y,   z,   s)?;
	read_cube(rdr, &mut cubes[1], x+s, y,   z,   s)?;
	read_cube(rdr, &mut cubes[2], x,   y+s, z,   s)?;
	read_cube(rdr, &mut cubes[3], x+s, y+s, z,   s)?;
	read_cube(rdr, &mut cubes[4], x,   y,   z+s, s)?;
	read_cube(rdr, &mut cubes[5], x+s, y,   z+s, s)?;
	read_cube(rdr, &mut cubes[6], x,   y+s, z+s, s)?;
	read_cube(rdr, &mut cubes[7], x+s, y+s, z+s, s)?;
	return Ok(())
}

#[allow(unused_variables)]
pub fn read_header(zdata: &[u8]) -> std::io::Result<()> {
	let data = {
		let mut data: Vec<u8> = Vec::new();
		let mut decoder = flate2::read::GzDecoder::new(zdata);
		decoder.read_to_end(&mut data)?;
		data
	};

	let mut rdr = Cursor::new(data);

	let mut magic: [u8; 4] = [0; 4];
	rdr.read(&mut magic[..])?;

	let engine = if magic == *b"OCTA" {
		Engine::Sauerbraten
	} else if magic == *b"TMAP" {
		Engine::Tesseract
	} else {
		panic!();
	};

	let version      = rdr.read_u32::<LittleEndian>().unwrap();
	println!("version {:?}", version);
	let headersize   = rdr.read_u32::<LittleEndian>().unwrap();
	let worldsize    = rdr.read_u32::<LittleEndian>().unwrap();
	let numents      = rdr.read_u32::<LittleEndian>().unwrap();
	let numpvs       = rdr.read_u32::<LittleEndian>().unwrap();
	let numlightmaps = match engine {
		Engine::Sauerbraten => rdr.read_u32::<LittleEndian>().unwrap(),
		Engine::Tesseract => 0
	};
	let blendmap     = rdr.read_u32::<LittleEndian>().unwrap();
	let numvars      = rdr.read_u32::<LittleEndian>().unwrap();
	let numvslots    = if version >= 30 {
		rdr.read_u32::<LittleEndian>().unwrap()
	} else {
		0
	};

	println!("{:?}", engine);

	for i in 0..numvars {
		let t = rdr.read_u8().unwrap();
		let l = rdr.read_u16::<LittleEndian>().unwrap();
		let mut buf = Vec::new();
		buf.resize(l as usize, 0u8);
		rdr.read(&mut buf[..]).unwrap();
		//let name = std::str::from_utf8(&mut buf[..]).unwrap();
		let name = buf;
		let v: Var = match t {
			0 => { Var::IVar(rdr.read_u32::<LittleEndian>().unwrap()) },
			1 => { Var::FVar(rdr.read_f32::<LittleEndian>().unwrap()) },
			2 => {
				let l = rdr.read_u16::<LittleEndian>().unwrap();
				let mut buf = Vec::new();
				buf.resize(l as usize, 0u8);
				rdr.read(&mut buf[..]).unwrap();
				let value = std::str::from_utf8(&mut buf[..]).unwrap();
				Var::SVar(value.to_string())
			},
			_ => panic!("var type {:?}", t)
		};
		println!("#{}: {:?} = {:?}", i, name, v);
	}

	{
		let l = rdr.read_u8().unwrap();
		let mut buf = Vec::new();
		buf.resize(l as usize, 0u8);
		rdr.read(&mut buf[..]).unwrap();
		let name = std::str::from_utf8(&mut buf[..]).unwrap();
		println!("gamemode: {}", name);
		let skip = rdr.read_u8().unwrap();
	}

	let eif       = rdr.read_u16::<LittleEndian>().unwrap();
	let extrasize = rdr.read_u16::<LittleEndian>().unwrap();
	let nummru    = rdr.read_u16::<LittleEndian>().unwrap();
	rdr.seek(SeekFrom::Current(nummru as i64 * 2)).unwrap();

	for i in 0..numents {
		let x = rdr.read_f32::<LittleEndian>().unwrap();
		let y = rdr.read_f32::<LittleEndian>().unwrap();
		let z = rdr.read_f32::<LittleEndian>().unwrap();
		let attr1 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr2 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr3 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr4 = rdr.read_i16::<LittleEndian>().unwrap();
		let attr5 = rdr.read_i16::<LittleEndian>().unwrap();
		let t        = rdr.read_u8().unwrap();
		let reserved = rdr.read_u8().unwrap();

		let entdata = (x, y, z, attr1, attr2, attr3, attr4, attr5, t, reserved);
		println!("{:?}", entdata);
	}

	let mut vslots: Vec<VSlot> = Vec::new();

	while vslots.len() < numvslots as usize {
		let changed = rdr.read_i32::<LittleEndian>().unwrap();
		println!("{}:{}/{}", vslots.len(), changed, numvslots);
		if changed < 0 {
			vslots.resize((vslots.len() as isize - changed as isize) as usize, VSlot::default());
		} else {
			let prev = rdr.read_i32::<LittleEndian>().unwrap();
			let mut vslot: VSlot = VSlot::default();
			read_vslot(&mut rdr, &mut vslot, changed as u32);
			println!("{:?}", vslot);
			vslots.push(vslot);
		}
	}

	let hw = (worldsize >> 1) as i32;
	read_children(&mut rdr, 0, 0, 0, hw);

	Ok(())
}
