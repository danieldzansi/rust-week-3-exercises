use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self.value {
            0..=0xfc => vec![self.value as u8],
            0xfd..=0xffff => {
                let mut bytes = vec![0xfd];
                bytes.extend_from_slice(&(self.value as u16).to_le_bytes());
                bytes
            }
            0x1_0000..=0xffff_ffff => {
                let mut bytes = vec![0xfe];
                bytes.extend_from_slice(&(self.value as u32).to_le_bytes());
                bytes
            }
            _ => {
                let mut bytes = vec![0xff];
                bytes.extend_from_slice(&self.value.to_le_bytes());
                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let prefix = *bytes.first().ok_or(BitcoinError::InsufficientBytes)?;

        let (value, consumed) = match prefix {
            0x00..=0xfc => (u64::from(prefix), 1),
            0xfd => {
                let raw: [u8; 2] = bytes
                    .get(1..3)
                    .ok_or(BitcoinError::InsufficientBytes)?
                    .try_into()
                    .map_err(|_| BitcoinError::InvalidFormat)?;
                let value = u64::from(u16::from_le_bytes(raw));
                if value < 0xfd {
                    return Err(BitcoinError::InvalidFormat);
                }
                (value, 3)
            }
            0xfe => {
                let raw: [u8; 4] = bytes
                    .get(1..5)
                    .ok_or(BitcoinError::InsufficientBytes)?
                    .try_into()
                    .map_err(|_| BitcoinError::InvalidFormat)?;
                let value = u64::from(u32::from_le_bytes(raw));
                if value <= 0xffff {
                    return Err(BitcoinError::InvalidFormat);
                }
                (value, 5)
            }
            0xff => {
                let raw: [u8; 8] = bytes
                    .get(1..9)
                    .ok_or(BitcoinError::InsufficientBytes)?
                    .try_into()
                    .map_err(|_| BitcoinError::InvalidFormat)?;
                let value = u64::from_le_bytes(raw);
                if value <= 0xffff_ffff {
                    return Err(BitcoinError::InvalidFormat);
                }
                (value, 9)
            }
        };

        Ok((Self::new(value), consumed))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let decoded = hex::decode(encoded).map_err(serde::de::Error::custom)?;
        let txid = decoded
            .try_into()
            .map_err(|bytes: Vec<u8>| serde::de::Error::invalid_length(bytes.len(), &"32 bytes"))?;
        Ok(Self(txid))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36);
        bytes.extend_from_slice(&self.txid.0);
        bytes.extend_from_slice(&self.vout.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let raw = bytes.get(..36).ok_or(BitcoinError::InsufficientBytes)?;
        let txid = raw[..32]
            .try_into()
            .map_err(|_| BitcoinError::InvalidFormat)?;
        let vout = u32::from_le_bytes(
            raw[32..36]
                .try_into()
                .map_err(|_| BitcoinError::InvalidFormat)?,
        );
        Ok((Self::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = CompactSize::new(self.bytes.len() as u64).to_bytes();
        bytes.extend_from_slice(&self.bytes);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (length, prefix_len) = CompactSize::from_bytes(bytes)?;
        let length = usize::try_from(length.value).map_err(|_| BitcoinError::InvalidFormat)?;
        let end = prefix_len
            .checked_add(length)
            .ok_or(BitcoinError::InvalidFormat)?;
        let script = bytes
            .get(prefix_len..end)
            .ok_or(BitcoinError::InsufficientBytes)?;
        Ok((Self::new(script.to_vec()), end))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.previous_output.to_bytes();
        bytes.extend(self.script_sig.to_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (previous_output, outpoint_len) = OutPoint::from_bytes(bytes)?;
        let (script_sig, script_len) = Script::from_bytes(&bytes[outpoint_len..])?;
        let sequence_start = outpoint_len + script_len;
        let sequence_end = sequence_start
            .checked_add(4)
            .ok_or(BitcoinError::InvalidFormat)?;
        let sequence_bytes = bytes
            .get(sequence_start..sequence_end)
            .ok_or(BitcoinError::InsufficientBytes)?;
        let sequence = u32::from_le_bytes(
            sequence_bytes
                .try_into()
                .map_err(|_| BitcoinError::InvalidFormat)?,
        );

        Ok((
            Self::new(previous_output, script_sig, sequence),
            sequence_end,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.version.to_le_bytes().to_vec();
        bytes.extend(CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            bytes.extend(input.to_bytes());
        }
        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let version_bytes = bytes.get(..4).ok_or(BitcoinError::InsufficientBytes)?;
        let version = u32::from_le_bytes(
            version_bytes
                .try_into()
                .map_err(|_| BitcoinError::InvalidFormat)?,
        );
        let (input_count, count_len) = CompactSize::from_bytes(&bytes[4..])?;
        let input_count =
            usize::try_from(input_count.value).map_err(|_| BitcoinError::InvalidFormat)?;
        let mut offset = 4 + count_len;
        let mut inputs = Vec::new();

        for _ in 0..input_count {
            let (input, consumed) = TransactionInput::from_bytes(
                bytes.get(offset..).ok_or(BitcoinError::InsufficientBytes)?,
            )?;
            offset = offset
                .checked_add(consumed)
                .ok_or(BitcoinError::InvalidFormat)?;
            inputs.push(input);
        }

        let lock_time_end = offset.checked_add(4).ok_or(BitcoinError::InvalidFormat)?;
        let lock_time_bytes = bytes
            .get(offset..lock_time_end)
            .ok_or(BitcoinError::InsufficientBytes)?;
        let lock_time = u32::from_le_bytes(
            lock_time_bytes
                .try_into()
                .map_err(|_| BitcoinError::InvalidFormat)?,
        );

        Ok((Self::new(version, inputs, lock_time), lock_time_end))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Inputs: {}", self.inputs.len())?;
        for (index, input) in self.inputs.iter().enumerate() {
            writeln!(f, "Input {index}:")?;
            writeln!(
                f,
                "  Previous Output Txid: {}",
                hex::encode(input.previous_output.txid.0)
            )?;
            writeln!(f, "  Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(f, "  ScriptSig Length: {}", input.script_sig.len())?;
            writeln!(f, "  ScriptSig: {}", hex::encode(&input.script_sig.bytes))?;
            writeln!(f, "  Sequence: {}", input.sequence)?;
        }
        write!(f, "Lock Time: {}", self.lock_time)
    }
}
