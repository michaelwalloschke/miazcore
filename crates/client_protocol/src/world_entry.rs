use flate2::{Decompress, FlushDecompress, Status};

use crate::ProtocolError;

pub const CMSG_PLAYER_LOGIN: u32 = 0x003d;
pub const SMSG_UPDATE_OBJECT: u16 = 0x00a9;
pub const SMSG_FORCE_RUN_SPEED_CHANGE: u16 = 0x00e2;
pub const CMSG_FORCE_RUN_SPEED_CHANGE_ACK: u32 = 0x00e3;
pub const SMSG_COMPRESSED_UPDATE_OBJECT: u16 = 0x01f6;
pub const SMSG_LOGIN_VERIFY_WORLD: u16 = 0x0236;
pub const SMSG_MOVE_UNSET_CAN_FLY: u16 = 0x0344;
pub const CMSG_MOVE_SET_CAN_FLY_ACK: u32 = 0x0345;
pub const SMSG_TIME_SYNC_REQ: u16 = 0x0390;
pub const CMSG_TIME_SYNC_RESP: u32 = 0x0391;

const MAX_UPDATE_BODY_SIZE: usize = 1024 * 1024;
const MAX_UPDATE_BLOCKS: u32 = 4096;
const MAX_GUID_LIST: u32 = 65_536;
const MAX_SPLINE_NODES: u32 = 32_768;

const UPDATE_TYPE_VALUES: u8 = 0;
const UPDATE_TYPE_MOVEMENT: u8 = 1;
const UPDATE_TYPE_CREATE_OBJECT: u8 = 2;
const UPDATE_TYPE_CREATE_OBJECT2: u8 = 3;
const UPDATE_TYPE_OUT_OF_RANGE: u8 = 4;
const UPDATE_TYPE_NEAR: u8 = 5;

const OBJECT_TYPE_PLAYER: u8 = 4;

const UPDATE_FLAG_SELF: u16 = 0x0001;
const UPDATE_FLAG_TRANSPORT: u16 = 0x0002;
const UPDATE_FLAG_HAS_TARGET: u16 = 0x0004;
const UPDATE_FLAG_UNKNOWN: u16 = 0x0008;
const UPDATE_FLAG_LOWGUID: u16 = 0x0010;
const UPDATE_FLAG_LIVING: u16 = 0x0020;
const UPDATE_FLAG_STATIONARY_POSITION: u16 = 0x0040;
const UPDATE_FLAG_VEHICLE: u16 = 0x0080;
const UPDATE_FLAG_POSITION: u16 = 0x0100;
const UPDATE_FLAG_ROTATION: u16 = 0x0200;
const UPDATE_FLAG_KNOWN: u16 = 0x03ff;
const REQUIRED_SELF_UPDATE_FLAGS: u16 =
    UPDATE_FLAG_SELF | UPDATE_FLAG_LIVING | UPDATE_FLAG_STATIONARY_POSITION;

const MOVEMENT_FLAG_ON_TRANSPORT: u32 = 0x0000_0200;
const MOVEMENT_FLAG_FALLING: u32 = 0x0000_1000;
const MOVEMENT_FLAG_SWIMMING: u32 = 0x0020_0000;
const MOVEMENT_FLAG_FLYING: u32 = 0x0200_0000;
const MOVEMENT_FLAG_SPLINE_ELEVATION: u32 = 0x0400_0000;
const MOVEMENT_FLAG_SPLINE_ENABLED: u32 = 0x0800_0000;

const MOVEMENT_FLAG2_ALWAYS_ALLOW_PITCHING: u16 = 0x0020;
const MOVEMENT_FLAG2_INTERPOLATED_MOVEMENT: u16 = 0x0400;

const SPLINE_FINAL_POINT: u32 = 0x0000_8000;
const SPLINE_FINAL_TARGET: u32 = 0x0001_0000;
const SPLINE_FINAL_ANGLE: u32 = 0x0002_0000;
const SPLINE_FINAL_FACING_MASK: u32 = SPLINE_FINAL_POINT | SPLINE_FINAL_TARGET | SPLINE_FINAL_ANGLE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldEntryLocation {
    map_id: u32,
    position: [f32; 3],
    orientation: f32,
}

impl WorldEntryLocation {
    #[must_use]
    pub const fn map_id(self) -> u32 {
        self.map_id
    }

    #[must_use]
    pub const fn position(self) -> [f32; 3] {
        self.position
    }

    #[must_use]
    pub const fn orientation(self) -> f32 {
        self.orientation
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcoreTransportInfo {
    guid: u64,
    position: [f32; 3],
    orientation: f32,
    time: u32,
    seat: i8,
    time2: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcoreJumpInfo {
    z_speed: f32,
    sin_angle: f32,
    cos_angle: f32,
    xy_speed: f32,
}

impl AcoreJumpInfo {
    #[must_use]
    pub const fn values(self) -> [f32; 4] {
        [self.z_speed, self.sin_angle, self.cos_angle, self.xy_speed]
    }
}

/// AzerothCore-compatible build-12340 movement state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcoreMovementInfo {
    flags: u32,
    flags2: u16,
    timestamp: u32,
    position: [f32; 3],
    orientation: f32,
    transport: Option<AcoreTransportInfo>,
    pitch: Option<f32>,
    fall_time_ms: u32,
    jump: Option<AcoreJumpInfo>,
    spline_elevation: Option<f32>,
}

impl AcoreMovementInfo {
    #[must_use]
    pub const fn flags(self) -> u32 {
        self.flags
    }

    #[must_use]
    pub const fn flags2(self) -> u16 {
        self.flags2
    }

    #[must_use]
    pub const fn timestamp(self) -> u32 {
        self.timestamp
    }

    #[must_use]
    pub const fn position(self) -> [f32; 3] {
        self.position
    }

    #[must_use]
    pub const fn orientation(self) -> f32 {
        self.orientation
    }

    #[must_use]
    pub const fn fall_time_ms(self) -> u32 {
        self.fall_time_ms
    }

    #[must_use]
    pub const fn jump(self) -> Option<AcoreJumpInfo> {
        self.jump
    }

    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: u32) -> Self {
        self.timestamp = timestamp;
        self
    }

    fn decode(cursor: &mut Cursor<'_>) -> Result<Self, ProtocolError> {
        let flags = cursor.u32()?;
        let flags2 = cursor.u16()?;
        let timestamp = cursor.u32()?;
        let position = cursor.vector3()?;
        let orientation = cursor.finite_f32()?;

        let transport = if flags & MOVEMENT_FLAG_ON_TRANSPORT != 0 {
            let guid = cursor.packed_guid()?;
            let position = cursor.vector3()?;
            let orientation = cursor.finite_f32()?;
            let time = cursor.u32()?;
            let seat = i8::from_le_bytes([cursor.u8()?]);
            let time2 = (flags2 & MOVEMENT_FLAG2_INTERPOLATED_MOVEMENT != 0)
                .then(|| cursor.u32())
                .transpose()?;
            Some(AcoreTransportInfo {
                guid,
                position,
                orientation,
                time,
                seat,
                time2,
            })
        } else {
            None
        };

        let has_pitch = flags & (MOVEMENT_FLAG_SWIMMING | MOVEMENT_FLAG_FLYING) != 0
            || flags2 & MOVEMENT_FLAG2_ALWAYS_ALLOW_PITCHING != 0;
        let pitch = has_pitch.then(|| cursor.finite_f32()).transpose()?;
        let fall_time_ms = cursor.u32()?;
        let jump = if flags & MOVEMENT_FLAG_FALLING != 0 {
            Some(AcoreJumpInfo {
                z_speed: cursor.finite_f32()?,
                sin_angle: cursor.finite_f32()?,
                cos_angle: cursor.finite_f32()?,
                xy_speed: cursor.finite_f32()?,
            })
        } else {
            None
        };
        let spline_elevation = (flags & MOVEMENT_FLAG_SPLINE_ELEVATION != 0)
            .then(|| cursor.finite_f32())
            .transpose()?;

        Ok(Self {
            flags,
            flags2,
            timestamp,
            position,
            orientation,
            transport,
            pitch,
            fall_time_ms,
            jump,
            spline_elevation,
        })
    }

    fn encode(self) -> Result<Vec<u8>, ProtocolError> {
        let mut output = Vec::with_capacity(96);
        output.extend_from_slice(&self.flags.to_le_bytes());
        output.extend_from_slice(&self.flags2.to_le_bytes());
        output.extend_from_slice(&self.timestamp.to_le_bytes());
        push_vector3(&mut output, self.position)?;
        push_f32(&mut output, self.orientation)?;

        if self.flags & MOVEMENT_FLAG_ON_TRANSPORT != 0 {
            let transport = self.transport.ok_or(ProtocolError::MalformedFrame)?;
            push_packed_guid(&mut output, transport.guid);
            push_vector3(&mut output, transport.position)?;
            push_f32(&mut output, transport.orientation)?;
            output.extend_from_slice(&transport.time.to_le_bytes());
            output.push(transport.seat.to_le_bytes()[0]);
            if self.flags2 & MOVEMENT_FLAG2_INTERPOLATED_MOVEMENT != 0 {
                output.extend_from_slice(
                    &transport
                        .time2
                        .ok_or(ProtocolError::MalformedFrame)?
                        .to_le_bytes(),
                );
            } else if transport.time2.is_some() {
                return Err(ProtocolError::MalformedFrame);
            }
        } else if self.transport.is_some() {
            return Err(ProtocolError::MalformedFrame);
        }

        let has_pitch = self.flags & (MOVEMENT_FLAG_SWIMMING | MOVEMENT_FLAG_FLYING) != 0
            || self.flags2 & MOVEMENT_FLAG2_ALWAYS_ALLOW_PITCHING != 0;
        if has_pitch {
            push_f32(
                &mut output,
                self.pitch.ok_or(ProtocolError::MalformedFrame)?,
            )?;
        } else if self.pitch.is_some() {
            return Err(ProtocolError::MalformedFrame);
        }

        output.extend_from_slice(&self.fall_time_ms.to_le_bytes());
        if self.flags & MOVEMENT_FLAG_FALLING != 0 {
            let jump = self.jump.ok_or(ProtocolError::MalformedFrame)?;
            for value in jump.values() {
                push_f32(&mut output, value)?;
            }
        } else if self.jump.is_some() {
            return Err(ProtocolError::MalformedFrame);
        }

        if self.flags & MOVEMENT_FLAG_SPLINE_ELEVATION != 0 {
            push_f32(
                &mut output,
                self.spline_elevation.ok_or(ProtocolError::MalformedFrame)?,
            )?;
        } else if self.spline_elevation.is_some() {
            return Err(ProtocolError::MalformedFrame);
        }
        Ok(output)
    }

    fn require_supported_self(self) -> Result<Self, ProtocolError> {
        if self.flags != 0
            || self.flags2 != 0
            || self.transport.is_some()
            || self.pitch.is_some()
            || self.jump.is_some()
            || self.spline_elevation.is_some()
        {
            return Err(ProtocolError::UnsupportedMovementState);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BootstrapSpeeds {
    values: [f32; 9],
}

impl BootstrapSpeeds {
    #[must_use]
    pub const fn values(self) -> [f32; 9] {
        self.values
    }

    #[must_use]
    pub const fn run(self) -> f32 {
        self.values[1]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoritativeSelfState {
    guid: u64,
    movement: AcoreMovementInfo,
    speeds: BootstrapSpeeds,
}

impl AuthoritativeSelfState {
    #[must_use]
    pub const fn guid(self) -> u64 {
        self.guid
    }

    #[must_use]
    pub const fn movement(self) -> AcoreMovementInfo {
        self.movement
    }

    #[must_use]
    pub const fn speeds(self) -> BootstrapSpeeds {
        self.speeds
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForceRunSpeedChange {
    guid: u64,
    counter: u32,
    run_speed: f32,
}

impl ForceRunSpeedChange {
    #[must_use]
    pub const fn guid(self) -> u64 {
        self.guid
    }

    #[must_use]
    pub const fn counter(self) -> u32 {
        self.counter
    }

    #[must_use]
    pub const fn run_speed(self) -> f32 {
        self.run_speed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsetCanFly {
    guid: u64,
    counter: u32,
}

impl UnsetCanFly {
    #[must_use]
    pub const fn guid(self) -> u64 {
        self.guid
    }

    #[must_use]
    pub const fn counter(self) -> u32 {
        self.counter
    }
}

/// Decode the exact 20-byte world-entry location.
///
/// # Errors
///
/// Returns [`ProtocolError::MalformedFrame`] when the payload is truncated,
/// contains non-finite coordinates, or has trailing bytes.
pub fn decode_login_verify_world(payload: &[u8]) -> Result<WorldEntryLocation, ProtocolError> {
    let mut cursor = Cursor::new(payload);
    let location = WorldEntryLocation {
        map_id: cursor.u32()?,
        position: cursor.vector3()?,
        orientation: cursor.finite_f32()?,
    };
    cursor.finish()?;
    Ok(location)
}

#[must_use]
pub fn encode_player_login(guid: u64) -> [u8; 8] {
    guid.to_le_bytes()
}

/// Decode one compressed or uncompressed update body and return its matching self block.
///
/// # Errors
///
/// Returns a protocol error when decompression exceeds the bounded update
/// budget or any update block is structurally malformed.
pub fn decode_authoritative_self_update(
    opcode: u16,
    payload: &[u8],
    selected_guid: u64,
) -> Result<Option<AuthoritativeSelfState>, ProtocolError> {
    let decompressed;
    let body = match opcode {
        SMSG_UPDATE_OBJECT => payload,
        SMSG_COMPRESSED_UPDATE_OBJECT => {
            decompressed = decompress_update(payload)?;
            decompressed.as_slice()
        }
        _ => return Err(ProtocolError::MalformedFrame),
    };
    parse_update_body(body, selected_guid)
}

/// Decode `AzerothCore`'s build-12340 run-speed control message.
///
/// # Errors
///
/// Returns a protocol error for malformed, non-finite, or non-positive input.
pub fn decode_force_run_speed_change(payload: &[u8]) -> Result<ForceRunSpeedChange, ProtocolError> {
    let mut cursor = Cursor::new(payload);
    let guid = cursor.packed_guid()?;
    let counter = cursor.u32()?;
    if cursor.u8()? != 0 {
        return Err(ProtocolError::MalformedFrame);
    }
    let run_speed = cursor.finite_f32()?;
    if run_speed <= 0.0 {
        return Err(ProtocolError::MalformedFrame);
    }
    cursor.finish()?;
    Ok(ForceRunSpeedChange {
        guid,
        counter,
        run_speed,
    })
}

/// Encode the matching run-speed acknowledgement.
///
/// # Errors
///
/// Returns a protocol error if the movement state contains a non-finite value.
pub fn encode_force_run_speed_change_ack(
    change: ForceRunSpeedChange,
    movement: AcoreMovementInfo,
) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = Vec::with_capacity(64);
    push_packed_guid(&mut payload, change.guid);
    payload.extend_from_slice(&change.counter.to_le_bytes());
    payload.extend_from_slice(&movement.encode()?);
    push_f32(&mut payload, change.run_speed)?;
    Ok(payload)
}

/// Decode the selected mover's no-flight control message.
///
/// # Errors
///
/// Returns a protocol error when the message is truncated or has trailing bytes.
pub fn decode_unset_can_fly(payload: &[u8]) -> Result<UnsetCanFly, ProtocolError> {
    let mut cursor = Cursor::new(payload);
    let request = UnsetCanFly {
        guid: cursor.packed_guid()?,
        counter: cursor.u32()?,
    };
    cursor.finish()?;
    Ok(request)
}

/// Encode an applied-false no-flight acknowledgement.
///
/// # Errors
///
/// Returns a protocol error if the movement state contains a non-finite value.
pub fn encode_move_set_can_fly_ack(
    request: UnsetCanFly,
    movement: AcoreMovementInfo,
) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = Vec::with_capacity(68);
    payload.extend_from_slice(&request.guid.to_le_bytes());
    payload.extend_from_slice(&request.counter.to_le_bytes());
    payload.extend_from_slice(&movement.encode()?);
    payload.extend_from_slice(&0_u32.to_le_bytes());
    Ok(payload)
}

/// Decode the server time-synchronization counter.
///
/// # Errors
///
/// Returns a protocol error unless the payload is exactly one little-endian `u32`.
pub fn decode_time_sync_request(payload: &[u8]) -> Result<u32, ProtocolError> {
    let mut cursor = Cursor::new(payload);
    let counter = cursor.u32()?;
    cursor.finish()?;
    Ok(counter)
}

/// Identify a selected-mover control family that this slice deliberately defers.
///
/// # Errors
///
/// Returns a protocol error when a recognized message does not contain its
/// required packed mover GUID prefix.
pub fn decode_unsupported_self_control_guid(
    opcode: u16,
    payload: &[u8],
) -> Result<Option<u64>, ProtocolError> {
    if !matches!(
        opcode,
        0x00de
            | 0x00df
            | 0x00e4
            | 0x00e6
            | 0x00e8
            | 0x00ea
            | 0x00ef
            | 0x00f2
            | 0x00f3
            | 0x00f4
            | 0x00f5
            | 0x02da
            | 0x02dc
            | 0x02de
            | 0x0343
            | 0x0381
            | 0x0383
            | 0x0516
    ) {
        return Ok(None);
    }
    let mut cursor = Cursor::new(payload);
    cursor.packed_guid().map(Some)
}

#[must_use]
pub fn encode_time_sync_response(counter: u32, client_time_ms: u32) -> [u8; 8] {
    let mut payload = [0_u8; 8];
    payload[..4].copy_from_slice(&counter.to_le_bytes());
    payload[4..].copy_from_slice(&client_time_ms.to_le_bytes());
    payload
}

fn decompress_update(payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let (size_bytes, compressed) = payload
        .split_at_checked(4)
        .ok_or(ProtocolError::MalformedFrame)?;
    let declared_size = usize::try_from(u32::from_le_bytes(
        size_bytes
            .try_into()
            .map_err(|_| ProtocolError::MalformedFrame)?,
    ))
    .map_err(|_| ProtocolError::MalformedFrame)?;
    if !(4..=MAX_UPDATE_BODY_SIZE).contains(&declared_size) || compressed.is_empty() {
        return Err(ProtocolError::MalformedFrame);
    }

    let mut output = vec![0_u8; declared_size];
    let mut decompressor = Decompress::new(true);
    let status = decompressor
        .decompress(compressed, &mut output, FlushDecompress::Finish)
        .map_err(|_| ProtocolError::MalformedFrame)?;
    let consumed =
        usize::try_from(decompressor.total_in()).map_err(|_| ProtocolError::MalformedFrame)?;
    let produced =
        usize::try_from(decompressor.total_out()).map_err(|_| ProtocolError::MalformedFrame)?;
    if status != Status::StreamEnd || consumed != compressed.len() || produced != declared_size {
        return Err(ProtocolError::MalformedFrame);
    }
    Ok(output)
}

fn parse_update_body(
    body: &[u8],
    selected_guid: u64,
) -> Result<Option<AuthoritativeSelfState>, ProtocolError> {
    if body.len() > MAX_UPDATE_BODY_SIZE {
        return Err(ProtocolError::MalformedFrame);
    }
    let mut cursor = Cursor::new(body);
    let block_count = cursor.u32()?;
    if block_count > MAX_UPDATE_BLOCKS {
        return Err(ProtocolError::MalformedFrame);
    }
    let mut found = None;

    for _ in 0..block_count {
        match cursor.u8()? {
            UPDATE_TYPE_VALUES => {
                let _ = cursor.packed_guid()?;
                consume_update_mask(&mut cursor)?;
            }
            UPDATE_TYPE_MOVEMENT => {
                let _ = cursor.packed_guid()?;
                let _ = parse_movement_block(&mut cursor)?;
            }
            update_type @ (UPDATE_TYPE_CREATE_OBJECT | UPDATE_TYPE_CREATE_OBJECT2) => {
                let guid = cursor.packed_guid()?;
                let object_type = cursor.u8()?;
                let movement = parse_movement_block(&mut cursor)?;
                consume_update_mask(&mut cursor)?;

                if movement.update_flags & UPDATE_FLAG_SELF != 0 && guid != selected_guid {
                    return Err(ProtocolError::MalformedFrame);
                }
                if guid == selected_guid {
                    if found.is_some()
                        || update_type != UPDATE_TYPE_CREATE_OBJECT2
                        || object_type != OBJECT_TYPE_PLAYER
                        || movement.update_flags != REQUIRED_SELF_UPDATE_FLAGS
                    {
                        return Err(ProtocolError::MalformedFrame);
                    }
                    let info = movement
                        .movement
                        .ok_or(ProtocolError::MalformedFrame)?
                        .require_supported_self()?;
                    let speeds = movement.speeds.ok_or(ProtocolError::MalformedFrame)?;
                    if speeds.run() <= 0.0 {
                        return Err(ProtocolError::MalformedFrame);
                    }
                    found = Some(AuthoritativeSelfState {
                        guid,
                        movement: info,
                        speeds,
                    });
                }
            }
            UPDATE_TYPE_OUT_OF_RANGE | UPDATE_TYPE_NEAR => {
                let count = cursor.u32()?;
                if count > MAX_GUID_LIST {
                    return Err(ProtocolError::MalformedFrame);
                }
                for _ in 0..count {
                    let _ = cursor.packed_guid()?;
                }
            }
            _ => return Err(ProtocolError::MalformedFrame),
        }
    }
    cursor.finish()?;
    Ok(found)
}

struct MovementBlock {
    update_flags: u16,
    movement: Option<AcoreMovementInfo>,
    speeds: Option<BootstrapSpeeds>,
}

fn parse_movement_block(cursor: &mut Cursor<'_>) -> Result<MovementBlock, ProtocolError> {
    let update_flags = cursor.u16()?;
    if update_flags & !UPDATE_FLAG_KNOWN != 0 {
        return Err(ProtocolError::MalformedFrame);
    }

    let mut movement = None;
    let mut speeds = None;
    if update_flags & UPDATE_FLAG_LIVING != 0 {
        let info = AcoreMovementInfo::decode(cursor)?;
        let mut values = [0_f32; 9];
        for value in &mut values {
            *value = cursor.finite_f32()?;
        }
        if info.flags & MOVEMENT_FLAG_SPLINE_ENABLED != 0 {
            consume_create_spline(cursor)?;
        }
        movement = Some(info);
        speeds = Some(BootstrapSpeeds { values });
    } else if update_flags & UPDATE_FLAG_POSITION != 0 {
        let _ = cursor.packed_guid()?;
        cursor.skip(8 * 4)?;
    } else if update_flags & UPDATE_FLAG_STATIONARY_POSITION != 0 {
        cursor.skip(4 * 4)?;
    }

    if update_flags & UPDATE_FLAG_UNKNOWN != 0 {
        cursor.skip(4)?;
    }
    if update_flags & UPDATE_FLAG_LOWGUID != 0 {
        cursor.skip(4)?;
    }
    if update_flags & UPDATE_FLAG_HAS_TARGET != 0 {
        let _ = cursor.packed_guid()?;
    }
    if update_flags & UPDATE_FLAG_TRANSPORT != 0 {
        cursor.skip(4)?;
    }
    if update_flags & UPDATE_FLAG_VEHICLE != 0 {
        cursor.skip(8)?;
    }
    if update_flags & UPDATE_FLAG_ROTATION != 0 {
        cursor.skip(8)?;
    }

    Ok(MovementBlock {
        update_flags,
        movement,
        speeds,
    })
}

fn consume_create_spline(cursor: &mut Cursor<'_>) -> Result<(), ProtocolError> {
    let flags = cursor.u32()?;
    let facing = flags & SPLINE_FINAL_FACING_MASK;
    match facing {
        0 => {}
        SPLINE_FINAL_ANGLE => cursor.skip(4)?,
        SPLINE_FINAL_TARGET => cursor.skip(8)?,
        SPLINE_FINAL_POINT => cursor.skip(12)?,
        _ => return Err(ProtocolError::MalformedFrame),
    }
    cursor.skip(4 * 7)?;
    let node_count = cursor.u32()?;
    if node_count > MAX_SPLINE_NODES {
        return Err(ProtocolError::MalformedFrame);
    }
    let node_bytes = usize::try_from(node_count)
        .ok()
        .and_then(|count| count.checked_mul(12))
        .ok_or(ProtocolError::MalformedFrame)?;
    cursor.skip(node_bytes)?;
    cursor.skip(1 + 12)?;
    Ok(())
}

fn consume_update_mask(cursor: &mut Cursor<'_>) -> Result<(), ProtocolError> {
    let word_count = usize::from(cursor.u8()?);
    let mut value_count = 0_usize;
    for _ in 0..word_count {
        value_count = value_count
            .checked_add(cursor.u32()?.count_ones() as usize)
            .ok_or(ProtocolError::MalformedFrame)?;
    }
    let value_bytes = value_count
        .checked_mul(4)
        .ok_or(ProtocolError::MalformedFrame)?;
    cursor.skip(value_bytes)
}

fn push_vector3(output: &mut Vec<u8>, values: [f32; 3]) -> Result<(), ProtocolError> {
    for value in values {
        push_f32(output, value)?;
    }
    Ok(())
}

fn push_f32(output: &mut Vec<u8>, value: f32) -> Result<(), ProtocolError> {
    if !value.is_finite() {
        return Err(ProtocolError::MalformedFrame);
    }
    output.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn push_packed_guid(output: &mut Vec<u8>, guid: u64) {
    let bytes = guid.to_le_bytes();
    let mut mask = 0_u8;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != 0 {
            mask |= 1 << index;
        }
    }
    output.push(mask);
    output.extend(
        bytes
            .into_iter()
            .enumerate()
            .filter_map(|(index, byte)| (mask & (1 << index) != 0).then_some(byte)),
    );
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(ProtocolError::MalformedFrame)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProtocolError::MalformedFrame)?;
        self.offset = end;
        Ok(value)
    }

    fn skip(&mut self, count: usize) -> Result<(), ProtocolError> {
        let _ = self.take(count)?;
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| ProtocolError::MalformedFrame)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| ProtocolError::MalformedFrame)?,
        ))
    }

    fn finite_f32(&mut self) -> Result<f32, ProtocolError> {
        let value = f32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| ProtocolError::MalformedFrame)?,
        );
        value
            .is_finite()
            .then_some(value)
            .ok_or(ProtocolError::MalformedFrame)
    }

    fn vector3(&mut self) -> Result<[f32; 3], ProtocolError> {
        Ok([self.finite_f32()?, self.finite_f32()?, self.finite_f32()?])
    }

    fn packed_guid(&mut self) -> Result<u64, ProtocolError> {
        let mask = self.u8()?;
        let mut bytes = [0_u8; 8];
        for (index, byte) in bytes.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                *byte = self.u8()?;
            }
        }
        Ok(u64::from_le_bytes(bytes))
    }

    fn finish(self) -> Result<(), ProtocolError> {
        (self.offset == self.bytes.len())
            .then_some(())
            .ok_or(ProtocolError::MalformedFrame)
    }
}

#[cfg(test)]
mod tests {
    use super::{AcoreJumpInfo, AcoreMovementInfo, Cursor, MOVEMENT_FLAG_FALLING};

    #[test]
    fn acore_movement_codec_preserves_integer_fall_time_and_jump_order() {
        let expected = AcoreMovementInfo {
            flags: MOVEMENT_FLAG_FALLING,
            flags2: 0,
            timestamp: 0x1122_3344,
            position: [1.25, -2.5, 3.75],
            orientation: 0.5,
            transport: None,
            pitch: None,
            fall_time_ms: 0x7fc0_0001,
            jump: Some(AcoreJumpInfo {
                z_speed: 4.0,
                sin_angle: 0.25,
                cos_angle: 0.75,
                xy_speed: 5.0,
            }),
            spline_elevation: None,
        };
        let encoded = expected.encode().unwrap();
        assert_eq!(&encoded[26..30], &0x7fc0_0001_u32.to_le_bytes());
        assert_eq!(&encoded[34..38], &0.25_f32.to_le_bytes());
        assert_eq!(&encoded[38..42], &0.75_f32.to_le_bytes());

        let mut cursor = Cursor::new(&encoded);
        assert_eq!(AcoreMovementInfo::decode(&mut cursor).unwrap(), expected);
        cursor.finish().unwrap();
    }
}
