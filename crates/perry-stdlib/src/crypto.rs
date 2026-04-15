//! Crypto module
//!
//! Native implementation of Node.js crypto module functions.
//! Provides hashing (sha256, md5), random byte generation, AES encryption,
//! and key derivation (pbkdf2, scrypt).

use perry_runtime::{js_string_from_bytes, StringHeader};
use md5::{Md5, Digest as Md5Digest};
use sha2::{Sha256, Digest as Sha256Digest};
use rand::RngCore;
use aes::Aes256;
use cbc::{Encryptor, Decryptor, cipher::{KeyIvInit, block_padding::Pkcs7, BlockEncryptMut, BlockDecryptMut}};
use base64::Engine as _;

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<Vec<u8>> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(bytes.to_vec())
}

/// Extract the raw bytes from a pointer that might be a Buffer, a
/// StringHeader, or anything that uses the `[u32 byte-length prefix][bytes]`
/// layout. StringHeader has `utf16_len` at offset 0 and `byte_len` at
/// offset 4; BufferHeader has `length` at offset 0 and `capacity` at
/// offset 4. Both have the payload bytes immediately after the 8-byte
/// header, and both store the byte count (in UTF-8 / as raw bytes) in
/// the same u32 slot for our purposes — but we pick the correct field
/// based on whether the pointer is a registered Buffer.
unsafe fn bytes_from_ptr(ptr: i64) -> Vec<u8> {
    let addr = ptr as usize;
    if addr < 0x1000 {
        return Vec::new();
    }
    if perry_runtime::buffer::is_registered_buffer(addr) {
        let buf = ptr as *const perry_runtime::buffer::BufferHeader;
        let len = (*buf).length as usize;
        let data = (buf as *const u8).add(std::mem::size_of::<perry_runtime::buffer::BufferHeader>());
        return std::slice::from_raw_parts(data, len).to_vec();
    }
    // Fall back to StringHeader layout — the common case for literal
    // strings passed to crypto functions.
    let hdr = ptr as *const StringHeader;
    let len = (*hdr).byte_len as usize;
    let data = (hdr as *const u8).add(std::mem::size_of::<StringHeader>());
    std::slice::from_raw_parts(data, len).to_vec()
}

/// Allocate a new Buffer, copy `bytes` into it, return the registered pointer.
unsafe fn alloc_buffer_from_slice(bytes: &[u8]) -> *mut perry_runtime::buffer::BufferHeader {
    let buf = perry_runtime::buffer::buffer_alloc(bytes.len() as u32);
    if buf.is_null() {
        return buf;
    }
    (*buf).length = bytes.len() as u32;
    let dst = perry_runtime::buffer::buffer_data_mut(buf);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
    buf
}

/// Create SHA256 hash of data
/// crypto.createHash('sha256').update(data).digest('hex') -> string
#[no_mangle]
pub unsafe extern "C" fn js_crypto_sha256(data_ptr: *const StringHeader) -> *mut StringHeader {
    let data = match string_from_header(data_ptr) {
        Some(d) => d,
        None => return std::ptr::null_mut(),
    };

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    let hex_str = hex::encode(result);

    js_string_from_bytes(hex_str.as_ptr(), hex_str.len() as u32)
}

/// SHA256 over arbitrary bytes. Input can be a Buffer or a string (both
/// share the same `[u32 len][u32 cap_or_utf16_len][bytes...]` header
/// layout up to the data pointer offset). Output is a Buffer holding the
/// 32-byte digest. Used by `.digest()` (no arg) — the SCRAM path in
/// `@perry/postgres` relies on this.
///
/// Pointer is passed as `i64` so the codegen can feed either a NaN-unboxed
/// Buffer handle or a StringHeader pointer through the same FFI slot.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_sha256_bytes(data_ptr: i64) -> *mut perry_runtime::buffer::BufferHeader {
    let bytes = bytes_from_ptr(data_ptr);
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    alloc_buffer_from_slice(&digest)
}

/// Create MD5 hash of data
/// crypto.createHash('md5').update(data).digest('hex') -> string
#[no_mangle]
pub unsafe extern "C" fn js_crypto_md5(data_ptr: *const StringHeader) -> *mut StringHeader {
    let data = match string_from_header(data_ptr) {
        Some(d) => d,
        None => return std::ptr::null_mut(),
    };

    let mut hasher = Md5::new();
    hasher.update(&data);
    let result = hasher.finalize();
    let hex_str = hex::encode(result);

    js_string_from_bytes(hex_str.as_ptr(), hex_str.len() as u32)
}

/// Generate random bytes and return as a Buffer
/// crypto.randomBytes(size) -> Buffer
#[no_mangle]
pub extern "C" fn js_crypto_random_bytes_buffer(size: f64) -> *mut perry_runtime::buffer::BufferHeader {
    let size = size as usize;
    if size == 0 || size > 1024 * 1024 {
        return perry_runtime::buffer::buffer_alloc(0);
    }

    let buf = perry_runtime::buffer::buffer_alloc(size as u32);
    unsafe {
        (*buf).length = size as u32;
        let data = perry_runtime::buffer::buffer_data_mut(buf);
        let mut bytes = std::slice::from_raw_parts_mut(data, size);
        rand::thread_rng().fill_bytes(&mut bytes);
    }
    buf
}

/// Generate random bytes and return as hex string
/// crypto.randomBytes(size).toString('hex') -> string
#[no_mangle]
pub extern "C" fn js_crypto_random_bytes_hex(size: f64) -> *mut StringHeader {
    let size = size as usize;
    if size == 0 || size > 1024 * 1024 {
        // Limit to 1MB
        return std::ptr::null_mut();
    }

    let mut bytes = vec![0u8; size];
    rand::thread_rng().fill_bytes(&mut bytes);
    let hex_str = hex::encode(&bytes);

    js_string_from_bytes(hex_str.as_ptr(), hex_str.len() as u32)
}

/// Generate a random UUID v4 using crypto-secure random
/// crypto.randomUUID() -> string
#[no_mangle]
pub extern "C" fn js_crypto_random_uuid() -> *mut StringHeader {
    let uuid = uuid::Uuid::new_v4();
    let uuid_str = uuid.to_string();
    js_string_from_bytes(uuid_str.as_ptr(), uuid_str.len() as u32)
}

/// Create HMAC-SHA256
/// crypto.createHmac('sha256', key).update(data).digest('hex') -> string
#[no_mangle]
pub unsafe extern "C" fn js_crypto_hmac_sha256(
    key_ptr: *const StringHeader,
    data_ptr: *const StringHeader,
) -> *mut StringHeader {
    use sha2::Sha256;
    use hmac::{Hmac, Mac};

    type HmacSha256 = Hmac<Sha256>;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => return std::ptr::null_mut(),
    };

    let data = match string_from_header(data_ptr) {
        Some(d) => d,
        None => return std::ptr::null_mut(),
    };

    let mut mac = match HmacSha256::new_from_slice(&key) {
        Ok(m) => m,
        Err(_) => return std::ptr::null_mut(),
    };

    mac.update(&data);
    let result = mac.finalize();
    let hex_str = hex::encode(result.into_bytes());

    js_string_from_bytes(hex_str.as_ptr(), hex_str.len() as u32)
}

/// HMAC-SHA-256 over arbitrary bytes, returning a Buffer. Used by
/// `.digest()` (no arg) for SCRAM-SHA-256 key derivation.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_hmac_sha256_bytes(
    key_ptr: i64,
    data_ptr: i64,
) -> *mut perry_runtime::buffer::BufferHeader {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;

    let key = bytes_from_ptr(key_ptr);
    let data = bytes_from_ptr(data_ptr);
    let mut mac = match HmacSha256::new_from_slice(&key) {
        Ok(m) => m,
        Err(_) => return perry_runtime::buffer::buffer_alloc(0),
    };
    mac.update(&data);
    let digest = mac.finalize().into_bytes();
    alloc_buffer_from_slice(&digest)
}

/// PBKDF2-HMAC-SHA-256 returning a Buffer. Counterpart of
/// `crypto.pbkdf2Sync(password, salt, iterations, keylen, 'sha256')`.
/// Accepts string or Buffer for both password and salt.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_pbkdf2_bytes(
    password_ptr: i64,
    salt_ptr: i64,
    iterations: f64,
    keylen: f64,
) -> *mut perry_runtime::buffer::BufferHeader {
    use pbkdf2::pbkdf2_hmac;
    let password = bytes_from_ptr(password_ptr);
    let salt = bytes_from_ptr(salt_ptr);
    let iter = iterations as u32;
    let klen = keylen as usize;
    let mut out = vec![0u8; klen];
    pbkdf2_hmac::<Sha256>(&password, &salt, iter, &mut out);
    alloc_buffer_from_slice(&out)
}

// Type aliases for AES-256-CBC
type Aes256CbcEnc = Encryptor<Aes256>;
type Aes256CbcDec = Decryptor<Aes256>;

/// AES-256-CBC encryption
/// crypto.createCipheriv('aes-256-cbc', key, iv) -> string (base64)
///
/// # Safety
/// All pointers must be valid StringHeader pointers.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_aes256_encrypt(
    data_ptr: *const StringHeader,
    key_ptr: *const StringHeader,
    iv_ptr: *const StringHeader,
) -> *mut StringHeader {
    let data = match string_from_header(data_ptr) {
        Some(d) => d,
        None => return std::ptr::null_mut(),
    };

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => return std::ptr::null_mut(),
    };

    let iv = match string_from_header(iv_ptr) {
        Some(i) => i,
        None => return std::ptr::null_mut(),
    };

    // Key must be 32 bytes for AES-256
    if key.len() != 32 {
        return std::ptr::null_mut();
    }

    // IV must be 16 bytes
    if iv.len() != 16 {
        return std::ptr::null_mut();
    }

    // Create encryptor
    let cipher = Aes256CbcEnc::new_from_slices(&key, &iv);
    let cipher = match cipher {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    // Calculate padded buffer size (next multiple of 16)
    let block_size = 16;
    let padded_len = ((data.len() / block_size) + 1) * block_size;
    let mut buf = vec![0u8; padded_len];
    buf[..data.len()].copy_from_slice(&data);

    // Encrypt with PKCS7 padding
    let ciphertext = match cipher.encrypt_padded_mut::<Pkcs7>(&mut buf, data.len()) {
        Ok(ct) => ct,
        Err(_) => return std::ptr::null_mut(),
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(ciphertext);

    js_string_from_bytes(b64.as_ptr(), b64.len() as u32)
}

/// AES-256-CBC decryption
/// crypto.createDecipheriv('aes-256-cbc', key, iv) -> string
///
/// # Safety
/// All pointers must be valid StringHeader pointers.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_aes256_decrypt(
    data_ptr: *const StringHeader,  // base64 encoded ciphertext
    key_ptr: *const StringHeader,
    iv_ptr: *const StringHeader,
) -> *mut StringHeader {
    let data_b64 = match string_from_header(data_ptr) {
        Some(d) => d,
        None => return std::ptr::null_mut(),
    };

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => return std::ptr::null_mut(),
    };

    let iv = match string_from_header(iv_ptr) {
        Some(i) => i,
        None => return std::ptr::null_mut(),
    };

    // Key must be 32 bytes for AES-256
    if key.len() != 32 {
        return std::ptr::null_mut();
    }

    // IV must be 16 bytes
    if iv.len() != 16 {
        return std::ptr::null_mut();
    }

    // Decode base64 ciphertext
    let mut ciphertext = match base64::engine::general_purpose::STANDARD.decode(&data_b64) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    // Create decryptor
    let cipher = Aes256CbcDec::new_from_slices(&key, &iv);
    let cipher = match cipher {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    // Decrypt with PKCS7 padding
    let plaintext = match cipher.decrypt_padded_mut::<Pkcs7>(&mut ciphertext) {
        Ok(p) => p,
        Err(_) => return std::ptr::null_mut(),
    };

    // Return as UTF-8 string
    let text = String::from_utf8_lossy(plaintext);
    js_string_from_bytes(text.as_ptr(), text.len() as u32)
}

/// PBKDF2 key derivation
/// crypto.pbkdf2Sync(password, salt, iterations, keyLength, 'sha256') -> Buffer (hex string)
///
/// # Safety
/// Pointers must be valid StringHeader pointers.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_pbkdf2(
    password_ptr: *const StringHeader,
    salt_ptr: *const StringHeader,
    iterations: f64,
    key_length: f64,
) -> *mut StringHeader {
    let password = match string_from_header(password_ptr) {
        Some(p) => p,
        None => return std::ptr::null_mut(),
    };

    let salt = match string_from_header(salt_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let iterations = iterations as u32;
    let key_length = key_length as usize;

    if key_length == 0 || key_length > 1024 {
        return std::ptr::null_mut();
    }

    // Derive key using PBKDF2 with SHA-256
    let mut output = vec![0u8; key_length];
    pbkdf2::pbkdf2_hmac::<Sha256>(&password, &salt, iterations, &mut output);

    let hex_str = hex::encode(&output);
    js_string_from_bytes(hex_str.as_ptr(), hex_str.len() as u32)
}

/// Scrypt key derivation
/// crypto.scryptSync(password, salt, keyLength) -> Buffer (hex string)
///
/// # Safety
/// Pointers must be valid StringHeader pointers.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_scrypt(
    password_ptr: *const StringHeader,
    salt_ptr: *const StringHeader,
    key_length: f64,
) -> *mut StringHeader {
    let password = match string_from_header(password_ptr) {
        Some(p) => p,
        None => return std::ptr::null_mut(),
    };

    let salt = match string_from_header(salt_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let key_length = key_length as usize;

    if key_length == 0 || key_length > 1024 {
        return std::ptr::null_mut();
    }

    // Use recommended scrypt parameters (N=16384, r=8, p=1)
    let params = scrypt::Params::new(14, 8, 1, key_length).unwrap_or_else(|_| {
        scrypt::Params::new(14, 8, 1, 32).unwrap()
    });

    let mut output = vec![0u8; key_length];
    if scrypt::scrypt(&password, &salt, &params, &mut output).is_err() {
        return std::ptr::null_mut();
    }

    let hex_str = hex::encode(&output);
    js_string_from_bytes(hex_str.as_ptr(), hex_str.len() as u32)
}

/// Scrypt key derivation with custom parameters
/// crypto.scryptSync(password, salt, keyLength, { N, r, p }) -> Buffer (hex string)
///
/// # Safety
/// Pointers must be valid StringHeader pointers.
#[no_mangle]
pub unsafe extern "C" fn js_crypto_scrypt_custom(
    password_ptr: *const StringHeader,
    salt_ptr: *const StringHeader,
    key_length: f64,
    log_n: f64,  // log2(N)
    r: f64,
    p: f64,
) -> *mut StringHeader {
    let password = match string_from_header(password_ptr) {
        Some(p) => p,
        None => return std::ptr::null_mut(),
    };

    let salt = match string_from_header(salt_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let key_length = key_length as usize;
    let log_n = log_n as u8;
    let r = r as u32;
    let p = p as u32;

    if key_length == 0 || key_length > 1024 {
        return std::ptr::null_mut();
    }

    let params = match scrypt::Params::new(log_n, r, p, key_length) {
        Ok(p) => p,
        Err(_) => return std::ptr::null_mut(),
    };

    let mut output = vec![0u8; key_length];
    if scrypt::scrypt(&password, &salt, &params, &mut output).is_err() {
        return std::ptr::null_mut();
    }

    let hex_str = hex::encode(&output);
    js_string_from_bytes(hex_str.as_ptr(), hex_str.len() as u32)
}
