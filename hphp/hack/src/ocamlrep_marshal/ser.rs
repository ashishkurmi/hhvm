#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(clippy::needless_late_init)]

// Initially generatd by c2rust of 'extern.c' at revision:
// `f14c8ff3f8a164685bc24184fba84904391e378e`.

use std::io::Write;

use libc::c_char;
use libc::c_double;
use libc::c_int;
use libc::c_long;
use libc::c_ulong;
use libc::c_void;
use libc::memcpy;
use ocamlrep::Header;
use ocamlrep::Value;

use crate::intext::*;

extern "C" {
    fn caml_alloc_string(len: mlsize_t) -> value;
}

type int64_t = c_long;
type intnat = c_long;
type uintnat = c_ulong;

type value = intnat;
type header_t = uintnat;
type mlsize_t = uintnat;
type color_t = uintnat;

#[inline]
const fn Is_long(x: Value<'_>) -> bool {
    x.is_immediate()
}
#[inline]
const fn Is_block(x: Value<'_>) -> bool {
    !x.is_immediate()
}

// Conversion macro names are always of the form  "to_from".
// Example: Val_long as in "Val from long" or "Val of long".

#[inline]
fn Long_val(x: Value<'_>) -> intnat {
    x.as_int().unwrap() as intnat
}
#[inline]
const fn Tag_hd(hd: Header) -> u8 {
    hd.tag()
}
#[inline]
const fn Wosize_hd(hd: Header) -> mlsize_t {
    hd.size() as mlsize_t
}
#[inline]
fn Hd_val(v: Value<'_>) -> Header {
    v.as_block().unwrap().header()
}
#[inline]
const fn Bsize_wsize(sz: mlsize_t) -> mlsize_t {
    sz.wrapping_mul(std::mem::size_of::<Value<'_>>() as mlsize_t)
}
#[inline]
const fn Bosize_hd(hd: Header) -> mlsize_t {
    Bsize_wsize(Wosize_hd(hd))
}
#[inline]
fn Tag_val(val: Value<'_>) -> u8 {
    val.as_block().unwrap().header().tag()
}

/// Fields are numbered from 0.
#[inline]
fn Field(x: Value<'_>, i: usize) -> Value<'_> {
    x.field(i).unwrap()
}
#[inline]
fn Field_ref<'a>(x: Value<'a>, i: usize) -> &'a Value<'a> {
    x.field_ref(i).unwrap()
}

#[inline]
fn Forward_val(v: Value<'_>) -> Value<'_> {
    Field(v, 0)
}
#[inline]
const fn Infix_offset_hd(hd: Header) -> mlsize_t {
    Bosize_hd(hd)
}

#[inline]
const fn Make_header(wosize: mlsize_t, tag: u8, color: color_t) -> header_t {
    (wosize << 10)
        .wrapping_add(color as header_t)
        .wrapping_add(tag as header_t)
}

// Flags affecting marshaling

const NO_SHARING: c_int = 1; // Flag to ignore sharing
const CLOSURES: c_int = 2; // Flag to allow marshaling code pointers
const COMPAT_32: c_int = 4; // Flag to ensure that output can safely be read back on a 32-bit platform

fn convert_flag_list(mut list: Value<'_>, flags: &[c_int]) -> Result<c_int, ocamlrep::FromError> {
    let mut res = 0;
    while !list.is_immediate() {
        let block = ocamlrep::from::expect_tuple(list, 2)?;
        let idx: usize = ocamlrep::from::field(block, 0)?;
        res |= flags[idx];
        list = block[1];
    }
    Ok(res)
}

// Stack for pending values to marshal

const EXTERN_STACK_INIT_SIZE: usize = 256;
const EXTERN_STACK_MAX_SIZE: usize = 1024 * 1024 * 100;

#[derive(Copy, Clone)]
#[repr(C)]
struct extern_item<'a> {
    v: &'a Value<'a>,
    count: mlsize_t,
}

// Hash table to record already-marshaled objects and their positions

#[derive(Copy, Clone)]
#[repr(C)]
struct object_position<'a> {
    obj: Value<'a>,
    pos: uintnat,
}

// The hash table uses open addressing, linear probing, and a redundant
// representation:
// - a bitvector [present] records which entries of the table are occupied;
// - an array [entries] records (object, position) pairs for the entries
//     that are occupied.
// The bitvector is much smaller than the array (1/128th on 64-bit
// platforms, 1/64th on 32-bit platforms), so it has better locality,
// making it faster to determine that an object is not in the table.
// Also, it makes it faster to empty or initialize a table: only the
// [present] bitvector needs to be filled with zeros, the [entries]
// array can be left uninitialized.

#[repr(C)]
struct position_table<'a> {
    shift: c_int,
    size: mlsize_t,                      // size == 1 << (wordsize - shift)
    mask: mlsize_t,                      // mask == size - 1
    threshold: mlsize_t,                 // threshold == a fixed fraction of size
    present: Box<[uintnat]>,             // [Bitvect_size(size)]
    entries: Box<[object_position<'a>]>, // [size]
}

const Bits_word: usize = 8 * std::mem::size_of::<uintnat>();

#[inline]
const fn Bitvect_size(n: usize) -> usize {
    (n + Bits_word - 1) / Bits_word
}

const POS_TABLE_INIT_SIZE_LOG2: usize = 8;
const POS_TABLE_INIT_SIZE: usize = 1 << POS_TABLE_INIT_SIZE_LOG2;

// Multiplicative Fibonacci hashing
// (Knuth, TAOCP vol 3, section 6.4, page 518).
// HASH_FACTOR is (sqrt(5) - 1) / 2 * 2^wordsize.
const HASH_FACTOR: uintnat = 11400714819323198486;
#[inline]
const fn Hash(v: Value<'_>, shift: c_int) -> mlsize_t {
    (v.to_bits() as uintnat).wrapping_mul(HASH_FACTOR) >> shift as uintnat
}

// When the table becomes 2/3 full, its size is increased.
#[inline]
const fn Threshold(sz: usize) -> usize {
    sz.wrapping_mul(2).wrapping_div(3)
}

// Accessing bitvectors

#[inline]
unsafe fn bitvect_test(bv: *mut uintnat, i: uintnat) -> uintnat {
    *bv.offset(i.wrapping_div(Bits_word as uintnat) as isize)
        & (1 << (i & (Bits_word - 1) as uintnat))
}

#[inline]
unsafe fn bitvect_set(bv: *mut uintnat, i: uintnat) {
    *bv.offset(i.wrapping_div(Bits_word as uintnat) as isize) |=
        1 << (i & (Bits_word - 1) as uintnat);
}

// Conversion to big-endian

#[inline]
fn store16(dst: &mut impl Write, n: c_int) {
    dst.write_all(&(n as i16).to_be_bytes()).unwrap()
}

#[inline]
fn store32(dst: &mut impl Write, n: intnat) {
    dst.write_all(&(n as i32).to_be_bytes()).unwrap()
}

#[inline]
fn store64(dst: &mut impl Write, n: int64_t) {
    dst.write_all(&(n as i64).to_be_bytes()).unwrap()
}

#[repr(C)]
struct State<'a> {
    extern_flags: c_int, // logical or of some of the flags

    obj_counter: uintnat, // Number of objects emitted so far
    size_32: uintnat,     // Size in words of 32-bit block for struct.
    size_64: uintnat,     // Size in words of 64-bit block for struct.

    // Stack for pending value to marshal
    stack: Vec<extern_item<'a>>,

    // Hash table to record already marshalled objects
    pos_table: position_table<'a>,

    // To buffer the output
    output: Vec<u8>,
}

impl<'a> State<'a> {
    fn new() -> Self {
        Self {
            extern_flags: 0,
            obj_counter: 0,
            size_32: 0,
            size_64: 0,

            stack: Vec::with_capacity(EXTERN_STACK_INIT_SIZE),

            pos_table: position_table {
                shift: 0,
                size: 0,
                mask: 0,
                threshold: 0,
                present: [].into(),
                entries: [].into(),
            },

            output: vec![],
        }
    }

    /// Initialize the position table
    unsafe fn init_position_table(&mut self) {
        if self.extern_flags & NO_SHARING != 0 {
            return;
        }
        self.pos_table.size = POS_TABLE_INIT_SIZE as mlsize_t;
        self.pos_table.shift = 8usize
            .wrapping_mul(std::mem::size_of::<Value<'a>>())
            .wrapping_sub(POS_TABLE_INIT_SIZE_LOG2) as c_int;
        self.pos_table.mask = (POS_TABLE_INIT_SIZE - 1) as mlsize_t;
        self.pos_table.threshold = Threshold(POS_TABLE_INIT_SIZE) as mlsize_t;
        self.pos_table.present =
            Box::new_zeroed_slice(Bitvect_size(POS_TABLE_INIT_SIZE)).assume_init();
        self.pos_table.entries = Box::new_uninit_slice(POS_TABLE_INIT_SIZE).assume_init();
    }

    /// Grow the position table
    unsafe fn resize_position_table(&mut self) {
        let new_size: mlsize_t;
        let new_shift: c_int;
        let new_present: Box<[uintnat]>;
        let new_entries: Box<[object_position<'a>]>;
        let mut i: uintnat;
        let mut h: uintnat;
        let mut old: position_table<'a>;

        // Grow the table quickly (x 8) up to 10^6 entries,
        // more slowly (x 2) afterwards.
        if self.pos_table.size < 1000000 {
            new_size = (self.pos_table.size).wrapping_mul(8);
            new_shift = self.pos_table.shift - 3;
        } else {
            new_size = (self.pos_table.size).wrapping_mul(2);
            new_shift = self.pos_table.shift - 1;
        }
        new_entries = Box::new_uninit_slice(new_size as usize).assume_init();
        new_present = Box::new_zeroed_slice(Bitvect_size(new_size as usize)).assume_init();
        old = std::mem::replace(
            &mut self.pos_table,
            position_table {
                size: new_size,
                shift: new_shift,
                mask: new_size.wrapping_sub(1),
                threshold: Threshold(new_size as usize) as mlsize_t,
                present: new_present,
                entries: new_entries,
            },
        );

        // Insert every entry of the old table in the new table
        let old_present = old.present.as_mut_ptr();
        let old_entries = old.entries.as_mut_ptr();
        let new_present = self.pos_table.present.as_mut_ptr();
        let new_entries = self.pos_table.entries.as_mut_ptr();
        i = 0;
        while i < old.size {
            if bitvect_test(old_present, i) != 0 {
                h = Hash((*old_entries.offset(i as isize)).obj, self.pos_table.shift);
                while bitvect_test(new_present, h) != 0 {
                    h = h.wrapping_add(1) & self.pos_table.mask
                }
                bitvect_set(new_present, h);
                *new_entries.offset(h as isize) = *old_entries.offset(i as isize)
            }
            i = i.wrapping_add(1)
        }
    }

    /// Determine whether the given object [obj] is in the hash table.
    /// If so, set `*pos_out` to its position in the output and return 1.
    /// If not, set `*h_out` to the hash value appropriate for
    /// `record_location` and return 0.
    #[inline]
    unsafe fn lookup_position(
        &mut self,
        obj: Value<'a>,
        pos_out: *mut uintnat,
        h_out: *mut uintnat,
    ) -> c_int {
        let mut h: uintnat = Hash(obj, self.pos_table.shift);
        loop {
            if bitvect_test(self.pos_table.present.as_mut_ptr(), h) == 0 {
                *h_out = h;
                return 0;
            }
            if (*self.pos_table.entries.as_mut_ptr().offset(h as isize)).obj == obj {
                *pos_out = (*self.pos_table.entries.as_mut_ptr().offset(h as isize)).pos;
                return 1;
            }
            h = h.wrapping_add(1) & self.pos_table.mask
        }
    }

    /// Record the output position for the given object [obj].
    ///
    /// The [h] parameter is the index in the hash table where the object
    /// must be inserted.  It was determined during lookup.
    unsafe fn record_location(&mut self, obj: Value<'a>, h: uintnat) {
        if self.extern_flags & NO_SHARING != 0 {
            return;
        }
        bitvect_set(self.pos_table.present.as_mut_ptr(), h);
        (*self.pos_table.entries.as_mut_ptr().offset(h as isize)).obj = obj;
        (*self.pos_table.entries.as_mut_ptr().offset(h as isize)).pos = self.obj_counter;
        self.obj_counter = self.obj_counter.wrapping_add(1);
        if self.obj_counter >= self.pos_table.threshold {
            self.resize_position_table();
        };
    }

    // To buffer the output

    fn init_output(&mut self) {
        self.output = Vec::with_capacity(SIZE_EXTERN_OUTPUT_BLOCK);
    }

    fn close_output(&mut self) {}

    fn output_length(&mut self) -> intnat {
        self.output.len() as intnat
    }

    // Panic raising (cleanup is handled by State's Drop impl)

    fn invalid_argument(&mut self, msg: &str) -> ! {
        panic!("{}", msg);
    }

    fn failwith(&mut self, msg: &str) -> ! {
        panic!("{}", msg);
    }

    fn stack_overflow(&mut self) -> ! {
        panic!("Stack overflow in marshaling value");
    }

    // Write characters, integers, and blocks in the output buffer

    #[inline]
    fn write(&mut self, c: c_int) {
        self.output.write_all(&[c as u8]).unwrap();
    }

    unsafe fn writeblock(&mut self, data: *const c_char, len: intnat) {
        self.output
            .write_all(std::slice::from_raw_parts(data as *const u8, len as usize))
            .unwrap();
    }

    #[inline]
    unsafe fn writeblock_float8(&mut self, data: *const c_double, ndoubles: intnat) {
        if ARCH_FLOAT_ENDIANNESS == 0x01234567 || ARCH_FLOAT_ENDIANNESS == 0x76543210 {
            self.writeblock(data as *const c_char, ndoubles * 8);
        } else {
            unimplemented!()
        }
    }

    fn writecode8(&mut self, code: c_int, val: intnat) {
        self.output.write_all(&[code as u8, val as u8]).unwrap();
    }

    fn writecode16(&mut self, code: c_int, val: intnat) {
        self.output.write_all(&[code as u8]).unwrap();
        store16(&mut self.output, val as c_int);
    }

    fn writecode32(&mut self, code: c_int, val: intnat) {
        self.output.write_all(&[code as u8]).unwrap();
        store32(&mut self.output, val);
    }

    fn writecode64(&mut self, code: c_int, val: intnat) {
        self.output.write_all(&[code as u8]).unwrap();
        store64(&mut self.output, val);
    }

    /// Marshaling integers
    #[inline]
    unsafe fn extern_int(&mut self, n: intnat) {
        if (0..0x40).contains(&n) {
            self.write(PREFIX_SMALL_INT + n as c_int);
        } else if (-(1 << 7)..(1 << 7)).contains(&n) {
            self.writecode8(CODE_INT8, n);
        } else if (-(1 << 15)..(1 << 15)).contains(&n) {
            self.writecode16(CODE_INT16, n);
        } else if !(-(1 << 30)..(1 << 30)).contains(&n) {
            if self.extern_flags & COMPAT_32 != 0 {
                self.failwith("output_value: integer cannot be read back on 32-bit platform");
            }
            self.writecode64(CODE_INT64, n);
        } else {
            self.writecode32(CODE_INT32, n);
        };
    }

    /// Marshaling references to previously-marshaled blocks
    #[inline]
    unsafe fn extern_shared_reference(&mut self, d: uintnat) {
        if d < 0x100 {
            self.writecode8(CODE_SHARED8, d as intnat);
        } else if d < 0x10000 {
            self.writecode16(CODE_SHARED16, d as intnat);
        } else if d >= 1 << 32 {
            self.writecode64(CODE_SHARED64, d as intnat);
        } else {
            self.writecode32(CODE_SHARED32, d as intnat);
        };
    }

    /// Marshaling block headers
    #[inline]
    unsafe fn extern_header(&mut self, sz: mlsize_t, tag: u8) {
        if tag < 16 && sz < 8 {
            self.write(
                PREFIX_SMALL_BLOCK
                    .wrapping_add(tag as c_int)
                    .wrapping_add((sz << 4) as c_int),
            );
        } else {
            // Note: ocaml-14.4.0 uses `Caml_white` (`0 << 8`)
            // ('caml/runtime/gc.h'). That's why we use this here, so that we
            // may test what this program generates vs. ocaml-4.14.0 in use
            // today.
            //
            // In PR https://github.com/ocaml/ocaml/pull/10831, in commit
            // `https://github.com/ocaml/ocaml/commit/868265f4532a2cc33bbffd83221c9613e743d759`
            // this becomes,
            //   let hd: header_t = Make_header(sz, tag, NOT_MARKABLE);
            // where, `NOT_MARKABLE` (`3 << 8`) ('caml/runtime/shared_heap.h').
            let hd: header_t = Make_header(sz, tag, 0 << 8);

            if sz > 0x3FFFFF && self.extern_flags & COMPAT_32 != 0 {
                self.failwith("output_value: array cannot be read back on 32-bit platform");
            }
            if hd < 1 << 32 {
                self.writecode32(CODE_BLOCK32, hd as intnat);
            } else {
                self.writecode64(CODE_BLOCK64, hd as intnat);
            }
        };
    }

    #[inline]
    unsafe fn extern_string(&mut self, bytes: &'a [u8]) {
        let len: intnat = bytes.len().try_into().unwrap();
        if len < 0x20 {
            self.write(PREFIX_SMALL_STRING.wrapping_add(len as c_int));
        } else if len < 0x100 {
            self.writecode8(CODE_STRING8, len);
        } else {
            if len > 0xFFFFFB && self.extern_flags & COMPAT_32 != 0 {
                self.failwith("output_value: string cannot be read back on 32-bit platform");
            }
            if len < 1 << 32 {
                self.writecode32(CODE_STRING32, len);
            } else {
                self.writecode64(CODE_STRING64, len);
            }
        }
        self.writeblock(bytes.as_ptr() as *const c_char, len);
    }

    /// Marshaling FP numbers
    #[inline]
    unsafe fn extern_double(&mut self, v: f64) {
        self.write(CODE_DOUBLE_NATIVE);
        self.writeblock_float8(&v, 1 as intnat);
    }

    /// Marshaling FP arrays
    #[inline]
    unsafe fn extern_double_array(&mut self, slice: &[f64]) {
        let nfloats = slice.len() as intnat;
        if nfloats < 0x100 {
            self.writecode8(CODE_DOUBLE_ARRAY8_NATIVE, nfloats);
        } else {
            if nfloats > 0x1FFFFF && self.extern_flags & COMPAT_32 != 0 {
                self.failwith("output_value: float array cannot be read back on 32-bit platform");
            }
            if nfloats < 1 << 32 {
                self.writecode32(CODE_DOUBLE_ARRAY32_NATIVE, nfloats);
            } else {
                self.writecode64(CODE_DOUBLE_ARRAY64_NATIVE, nfloats);
            }
        }
        self.writeblock_float8(slice.as_ptr() as *const c_double, nfloats);
    }

    /// Marshal the given value in the output buffer
    unsafe fn extern_rec(&mut self, mut v: Value<'a>) {
        let mut goto_next_item: bool;

        let mut h: uintnat = 0;
        let mut pos: uintnat = 0;

        self.init_position_table();

        loop {
            if Is_long(v) {
                self.extern_int(Long_val(v));
            } else {
                let hd: Header = Hd_val(v);
                let tag: u8 = Tag_hd(hd);
                let sz: mlsize_t = Wosize_hd(hd);

                if tag == ocamlrep::FORWARD_TAG {
                    let f: Value<'a> = Forward_val(v);
                    if Is_block(f)
                        && (Tag_val(f) == ocamlrep::FORWARD_TAG
                            || Tag_val(f) == ocamlrep::LAZY_TAG
                            || Tag_val(f) == ocamlrep::FORCING_TAG
                            || Tag_val(f) == ocamlrep::DOUBLE_TAG)
                    {
                        // Do not short-circuit the pointer.
                    } else {
                        v = f;
                        continue;
                    }
                }
                // Atoms are treated specially for two reasons: they are not allocated
                // in the externed block, and they are automatically shared.
                if sz == 0 {
                    self.extern_header(0, tag);
                } else {
                    // Check if object already seen
                    if self.extern_flags & NO_SHARING == 0 {
                        if self.lookup_position(v, &mut pos, &mut h) != 0 {
                            self.extern_shared_reference(self.obj_counter.wrapping_sub(pos));
                            goto_next_item = true;
                        } else {
                            goto_next_item = false;
                        }
                    } else {
                        goto_next_item = false;
                    }
                    if !goto_next_item {
                        // Output the contents of the object
                        match tag {
                            ocamlrep::STRING_TAG => {
                                let bytes = v.as_byte_string().unwrap();
                                let len: mlsize_t = bytes.len() as mlsize_t;
                                self.extern_string(bytes);
                                self.size_32 = self.size_32.wrapping_add(
                                    (1 as uintnat)
                                        .wrapping_add(len.wrapping_add(4).wrapping_div(4)),
                                );
                                self.size_64 = self.size_64.wrapping_add(
                                    (1 as uintnat)
                                        .wrapping_add(len.wrapping_add(8).wrapping_div(8)),
                                );
                                self.record_location(v, h);
                            }
                            ocamlrep::DOUBLE_TAG => {
                                self.extern_double(v.as_float().unwrap());
                                self.size_32 = self.size_32.wrapping_add(1 + 2);
                                self.size_64 = self.size_64.wrapping_add(1 + 1);
                                self.record_location(v, h);
                            }
                            ocamlrep::DOUBLE_ARRAY_TAG => {
                                let slice = v.as_double_array().unwrap();
                                self.extern_double_array(slice);
                                let nfloats = slice.len() as mlsize_t;
                                self.size_32 = (*self).size_32.wrapping_add(
                                    (1 as uintnat).wrapping_add(nfloats.wrapping_mul(2)),
                                );
                                self.size_64 = (*self)
                                    .size_64
                                    .wrapping_add((1 as uintnat).wrapping_add(nfloats));
                                self.record_location(v, h);
                            }
                            ocamlrep::ABSTRACT_TAG => {
                                self.invalid_argument("output_value: abstract value (Abstract)");
                            }
                            ocamlrep::INFIX_TAG => {
                                self.writecode32(CODE_INFIXPOINTER, Infix_offset_hd(hd) as intnat);
                                v = Value::from_bits(
                                    (v.to_bits() as uintnat).wrapping_sub(Infix_offset_hd(hd))
                                        as usize,
                                ); // PR#5772
                                continue;
                            }
                            ocamlrep::CUSTOM_TAG => self.invalid_argument(
                                "output_value: marshaling of custom blocks not implemented",
                            ),
                            ocamlrep::CLOSURE_TAG => self.invalid_argument(
                                "output_value: marshaling of closures not implemented",
                            ),
                            _ => {
                                self.extern_header(sz, tag);
                                self.size_32 =
                                    self.size_32.wrapping_add((1 as uintnat).wrapping_add(sz));
                                self.size_64 =
                                    self.size_64.wrapping_add((1 as uintnat).wrapping_add(sz));
                                self.record_location(v, h);
                                // Remember that we still have to serialize fields 1 ... sz - 1
                                if sz > 1 {
                                    if self.stack.len() + 1 >= EXTERN_STACK_MAX_SIZE {
                                        self.stack_overflow();
                                    }
                                    self.stack.push(extern_item {
                                        v: Field_ref(v, 1),
                                        count: sz.wrapping_sub(1),
                                    });
                                }
                                // Continue serialization with the first field
                                v = Field(v, 0);
                                continue;
                            }
                        }
                    }
                }
            }
            // C goto label `next_item:` here

            // Pop one more item to marshal, if any
            if let Some(item) = self.stack.last_mut() {
                let fresh8 = item.v;
                item.v = &*(item.v as *const Value<'a>).offset(1) as &'a Value<'a>;
                v = *fresh8;
                item.count = item.count.wrapping_sub(1);
                if item.count == 0 {
                    self.stack.pop();
                }
            } else {
                // We are done.
                return;
            }
        }
        // Never reached as function leaves with return
    }

    unsafe fn extern_value(
        &mut self,
        v: Value<'a>,
        flags: Value<'a>,
        mut header: &mut [u8],  // out
        header_len: &mut usize, // out
    ) -> intnat {
        static EXTERN_FLAG_VALUES: [c_int; 3] = [NO_SHARING, CLOSURES, COMPAT_32];

        let res_len: intnat;
        // Parse flag list
        self.extern_flags = convert_flag_list(flags, &EXTERN_FLAG_VALUES).unwrap();
        // Initializations
        self.obj_counter = 0;
        self.size_32 = 0;
        self.size_64 = 0;
        // Marshal the object
        self.extern_rec(v);
        // Record end of output
        self.close_output();
        // Write the header
        res_len = self.output_length();
        if res_len >= (1 << 32) || self.size_32 >= (1 << 32) || self.size_64 >= (1 << 32) {
            // The object is too big for the small header format.
            // Fail if we are in compat32 mode, or use big header.
            if self.extern_flags & COMPAT_32 != 0 {
                self.failwith("output_value: object too big to be read back on 32-bit platform");
            }
            store32(&mut header, MAGIC_NUMBER_BIG as intnat);
            store32(&mut header, 0);
            store64(&mut header, res_len);
            store64(&mut header, self.obj_counter as int64_t);
            store64(&mut header, self.size_64 as int64_t);
            *header_len = 32;
            return res_len;
        }
        // Use the small header format
        store32(&mut header, MAGIC_NUMBER_SMALL as intnat);
        store32(&mut header, res_len);
        store32(&mut header, self.obj_counter as intnat);
        store32(&mut header, self.size_32 as intnat);
        store32(&mut header, self.size_64 as intnat);
        *header_len = 20;
        res_len
    }
}

unsafe fn output_val<W: std::io::Write>(
    w: &mut W,
    v: Value<'_>,
    flags: Value<'_>,
) -> std::io::Result<()> {
    let mut header: [u8; 32] = [0; 32];
    let mut header_len = 0;
    let mut s: State<'_> = State::new();
    s.init_output();
    s.extern_value(v, flags, &mut header, &mut header_len);
    let output = std::mem::take(&mut s.output);
    drop(s);
    w.write_all(&header[0..header_len])?;
    w.write_all(&output)?;
    w.flush()
}

#[no_mangle]
pub unsafe extern "C" fn ocamlrep_marshal_output_value_to_string(v: value, flags: value) -> value {
    let v = Value::from_bits(v as usize);
    let flags = Value::from_bits(flags as usize);
    let mut vec = vec![];
    output_val(&mut vec, v, flags).unwrap();
    let res: value = caml_alloc_string(vec.len() as mlsize_t);
    memcpy(res as *mut c_void, vec.as_ptr() as *const c_void, vec.len());
    res
}
