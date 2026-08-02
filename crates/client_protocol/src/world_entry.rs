use flate2::{Decompress, FlushDecompress, Status};

use crate::{ProtocolError, WorldServerFrame};

pub const CMSG_PLAYER_LOGIN: u32 = 0x003d;
/// Request a saving logout from the Reference Realm.  A successful movement
/// proof still requires a later fresh-world observation; this opcode is only
/// the lifecycle boundary that asks the realm to persist the current player.
pub const CMSG_LOGOUT_REQUEST: u32 = 0x004b;
pub const SMSG_LOGOUT_RESPONSE: u16 = 0x004c;
pub const SMSG_LOGOUT_COMPLETE: u16 = 0x004d;
/// Build-12340 movement opcodes are message opcodes: client and server share
/// the same numeric value, while the direction is determined by the frame
/// header.
pub const MSG_MOVE_START_FORWARD: u32 = 0x00b5;
pub const MSG_MOVE_STOP: u32 = 0x00b7;
pub const MSG_MOVE_HEARTBEAT: u32 = 0x00ee;
const MSG_MOVE_START_FORWARD_SERVER: u16 = 0x00b5;
const MSG_MOVE_STOP_SERVER: u16 = 0x00b7;
const MSG_MOVE_HEARTBEAT_SERVER: u16 = 0x00ee;
pub const SMSG_UPDATE_OBJECT: u16 = 0x00a9;
pub const SMSG_DESTROY_OBJECT: u16 = 0x00aa;
pub const SMSG_FORCE_RUN_SPEED_CHANGE: u16 = 0x00e2;
pub const CMSG_FORCE_RUN_SPEED_CHANGE_ACK: u32 = 0x00e3;
pub const SMSG_FORCE_MOVE_ROOT: u16 = 0x00e8;
pub const CMSG_FORCE_MOVE_ROOT_ACK: u32 = 0x00e9;
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
const MOVEMENT_FLAG_ROOT: u32 = 0x0000_0800;
const MOVEMENT_FLAG_FORWARD: u32 = 0x0000_0001;
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
    /// Construct the deliberately narrow, on-ground movement state used by
    /// the Learning Client.  Transport, falling, swimming, flying, pitch and
    /// spline states remain unsupported by this capability.
    #[must_use]
    pub const fn ground(
        timestamp: u32,
        position: [f32; 3],
        orientation: f32,
        moving_forward: bool,
    ) -> Self {
        Self {
            flags: if moving_forward {
                MOVEMENT_FLAG_FORWARD
            } else {
                0
            },
            flags2: 0,
            timestamp,
            position,
            orientation,
            transport: None,
            pitch: None,
            fall_time_ms: 0,
            jump: None,
            spline_elevation: None,
        }
    }
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

    /// Mark this otherwise on-ground state as rooted for a matching forced-root
    /// acknowledgement. This is deliberately not a general movement mode.
    #[must_use]
    pub const fn rooted(mut self) -> Self {
        self.flags |= MOVEMENT_FLAG_ROOT;
        self
    }

    /// Encode a validated outbound movement payload for a client movement
    /// message.  This does not frame or encrypt the packet.
    ///
    /// # Errors
    ///
    /// Returns an error if the movement state cannot be represented safely.
    pub fn encode_for_client(self) -> Result<Vec<u8>, ProtocolError> {
        self.encode()
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

    fn is_ordinary_ground(self) -> bool {
        self.flags
            & (MOVEMENT_FLAG_ON_TRANSPORT
                | MOVEMENT_FLAG_FALLING
                | MOVEMENT_FLAG_SWIMMING
                | MOVEMENT_FLAG_FLYING
                | MOVEMENT_FLAG_SPLINE_ELEVATION
                | MOVEMENT_FLAG_SPLINE_ENABLED)
            == 0
            && self.flags2
                & (MOVEMENT_FLAG2_ALWAYS_ALLOW_PITCHING | MOVEMENT_FLAG2_INTERPOLATED_MOVEMENT)
                == 0
            && self.transport.is_none()
            && self.pitch.is_none()
            && self.jump.is_none()
            && self.spline_elevation.is_none()
    }
}

/// Encode one complete client movement body: active mover packed GUID followed
/// by its validated `MovementInfo` block.
///
/// # Errors
///
/// Returns an error when the movement payload contains invalid scalars.
pub fn encode_client_movement(
    active_mover: u64,
    movement: AcoreMovementInfo,
) -> Result<Vec<u8>, ProtocolError> {
    let movement = movement.encode()?;
    let mut output = Vec::with_capacity(9 + movement.len());
    push_packed_guid(&mut output, active_mover);
    output.extend_from_slice(&movement);
    Ok(output)
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

/// A non-owning, semantic record extracted for the reset-scoped remote-player
/// research tracer.
///
/// This type deliberately has no object-field or display metadata. Callers
/// must first establish encrypted frame integrity, then decide which player
/// GUIDs to retain. It is not a general object registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RemoteWorldTraceEvent {
    /// A non-self player create carrying an initial movement snapshot.
    PlayerCreate {
        guid: u64,
        movement: AcoreMovementInfo,
    },
    /// A GUID-prefixed movement message. Its object type is intentionally not
    /// inferred; the caller must accept it only for a prior player create.
    Movement {
        guid: u64,
        movement: AcoreMovementInfo,
        opcode: u16,
    },
    /// A GUID in a complete out-of-range list.
    OutOfRange { guid: u64 },
    /// A GUID in a complete object-destruction packet.
    Destroy { guid: u64 },
}

/// Bounded semantic output from a complete, authenticated World frame.
///
/// This deliberately exposes neither update values nor object metadata.  A
/// later session boundary owns peer selection and associates an authenticated
/// entry map with an accepted GUID.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RemotePlayerRecord {
    PlayerCreate {
        guid: u64,
        movement: AcoreMovementInfo,
    },
    PlayerMovement {
        guid: u64,
        movement: AcoreMovementInfo,
        opcode: u16,
    },
    OutOfRange {
        guid: u64,
    },
    Destroy {
        guid: u64,
    },
    UnusableMovement {
        guid: u64,
        category: RemotePlayerUnusableCategory,
    },
}

impl RemotePlayerRecord {
    #[must_use]
    pub const fn guid(self) -> u64 {
        match self {
            Self::PlayerCreate { guid, .. }
            | Self::PlayerMovement { guid, .. }
            | Self::OutOfRange { guid }
            | Self::Destroy { guid }
            | Self::UnusableMovement { guid, .. } => guid,
        }
    }
}

/// Redacted reason why a structurally complete peer movement cannot become a pose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemotePlayerUnusableCategory {
    InvalidPose,
    UnsupportedMovement,
}

impl RemoteWorldTraceEvent {
    #[must_use]
    pub const fn guid(self) -> u64 {
        match self {
            Self::PlayerCreate { guid, .. }
            | Self::Movement { guid, .. }
            | Self::OutOfRange { guid }
            | Self::Destroy { guid } => guid,
        }
    }
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

/// Server request that the active mover acknowledge a forced root state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForceMoveRoot {
    guid: u64,
    counter: u32,
}

impl ForceMoveRoot {
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
/// Returns [`ProtocolError::MalformedWorldEntry`] when the payload is truncated,
/// contains non-finite coordinates, or has trailing bytes.
pub fn decode_login_verify_world(payload: &[u8]) -> Result<WorldEntryLocation, ProtocolError> {
    let mut cursor = Cursor::new(payload, SMSG_LOGIN_VERIFY_WORLD);
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
        _ => return Err(malformed_world_entry(opcode, 0)),
    };
    parse_update_body(body, selected_guid, opcode)
}

/// Extract only the build-12340 records needed by the reset-scoped
/// remote-player semantic tracer.
///
/// The caller supplies complete plaintext World frames from the directional
/// frame decoder. Unknown opcodes yield no events; update containers are still
/// consumed completely so unrelated objects cannot disturb subsequent blocks.
///
/// # Errors
///
/// Returns an error when a supported record is malformed, compressed beyond
/// its bounded declaration, or cannot be structurally consumed.
pub fn decode_remote_world_trace(
    opcode: u16,
    payload: &[u8],
) -> Result<Vec<RemoteWorldTraceEvent>, ProtocolError> {
    let events = decode_remote_player_payload(opcode, payload)?
        .into_iter()
        .filter_map(|record| match record {
            RemotePlayerRecord::PlayerCreate { guid, movement } => {
                Some(RemoteWorldTraceEvent::PlayerCreate { guid, movement })
            }
            RemotePlayerRecord::PlayerMovement {
                guid,
                movement,
                opcode,
            } => Some(RemoteWorldTraceEvent::Movement {
                guid,
                movement,
                opcode,
            }),
            RemotePlayerRecord::OutOfRange { guid } => {
                Some(RemoteWorldTraceEvent::OutOfRange { guid })
            }
            RemotePlayerRecord::Destroy { guid } => Some(RemoteWorldTraceEvent::Destroy { guid }),
            RemotePlayerRecord::UnusableMovement { .. } => None,
        })
        .collect();
    Ok(events)
}

/// Decode the deliberately small remote-player vocabulary from one complete
/// plaintext World frame.
///
/// The frame must come from [`crate::IncrementalWorldServerDecoder`]. This
/// function does not accept byte chunks, headers, or cipher state, so it
/// cannot alter framing alignment while it consumes update containers.
///
/// # Errors
///
/// Returns an error when a relevant complete frame is malformed, exceeds the
/// bounded compression contract, or leaves trailing bytes. Structurally valid
/// but unusable movement yields a redacted record instead.
pub fn decode_remote_player_frame(
    frame: &WorldServerFrame,
) -> Result<Vec<RemotePlayerRecord>, ProtocolError> {
    decode_remote_player_payload(frame.opcode(), frame.payload())
}

fn decode_remote_player_payload(
    opcode: u16,
    payload: &[u8],
) -> Result<Vec<RemotePlayerRecord>, ProtocolError> {
    match opcode {
        SMSG_UPDATE_OBJECT => parse_remote_player_update_body(payload, opcode),
        SMSG_COMPRESSED_UPDATE_OBJECT => {
            let body = decompress_update(payload)?;
            parse_remote_player_update_body(&body, opcode)
        }
        MSG_MOVE_START_FORWARD_SERVER | MSG_MOVE_HEARTBEAT_SERVER | MSG_MOVE_STOP_SERVER => {
            let mut cursor = Cursor::new(payload, opcode);
            let guid = cursor.packed_guid()?;
            let (movement, _spline_enabled) = parse_remote_movement_info(&mut cursor)?;
            cursor.finish()?;
            Ok(remote_movement_record(guid, movement, opcode)
                .into_iter()
                .collect())
        }
        SMSG_DESTROY_OBJECT => {
            let mut cursor = Cursor::new(payload, opcode);
            let guid = cursor.u64()?;
            let _death = cursor.u8()?;
            cursor.finish()?;
            Ok((guid != 0)
                .then_some(RemotePlayerRecord::Destroy { guid })
                .into_iter()
                .collect())
        }
        _ => Ok(Vec::new()),
    }
}

/// Decode `AzerothCore`'s build-12340 run-speed control message.
///
/// # Errors
///
/// Returns a protocol error for malformed, non-finite, or non-positive input.
pub fn decode_force_run_speed_change(payload: &[u8]) -> Result<ForceRunSpeedChange, ProtocolError> {
    let mut cursor = Cursor::new(payload, SMSG_FORCE_RUN_SPEED_CHANGE);
    let guid = cursor.packed_guid()?;
    let counter = cursor.u32()?;
    if cursor.u8()? != 0 {
        return Err(cursor.malformed());
    }
    let run_speed = cursor.finite_f32()?;
    if run_speed <= 0.0 {
        return Err(cursor.malformed());
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
    let mut cursor = Cursor::new(payload, SMSG_MOVE_UNSET_CAN_FLY);
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

/// Decode the active mover's forced-root request.
///
/// # Errors
///
/// Returns a protocol error when the packet is truncated or has trailing bytes.
pub fn decode_force_move_root(payload: &[u8]) -> Result<ForceMoveRoot, ProtocolError> {
    let mut cursor = Cursor::new(payload, SMSG_FORCE_MOVE_ROOT);
    let request = ForceMoveRoot {
        guid: cursor.packed_guid()?,
        counter: cursor.u32()?,
    };
    cursor.finish()?;
    Ok(request)
}

/// Encode the matching forced-root acknowledgement with rooted movement state.
///
/// # Errors
///
/// Returns a protocol error if the movement state contains a non-finite value.
pub fn encode_force_move_root_ack(
    request: ForceMoveRoot,
    movement: AcoreMovementInfo,
) -> Result<Vec<u8>, ProtocolError> {
    let mut payload = Vec::with_capacity(64);
    push_packed_guid(&mut payload, request.guid);
    payload.extend_from_slice(&request.counter.to_le_bytes());
    payload.extend_from_slice(&movement.rooted().encode()?);
    Ok(payload)
}

/// Decode the server time-synchronization counter.
///
/// # Errors
///
/// Returns a protocol error unless the payload is exactly one little-endian `u32`.
pub fn decode_time_sync_request(payload: &[u8]) -> Result<u32, ProtocolError> {
    let mut cursor = Cursor::new(payload, SMSG_TIME_SYNC_REQ);
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
    let mut cursor = Cursor::new(payload, opcode);
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
        .ok_or_else(|| malformed_world_entry(SMSG_COMPRESSED_UPDATE_OBJECT, payload.len()))?;
    let declared_size = usize::try_from(u32::from_le_bytes(
        size_bytes
            .try_into()
            .map_err(|_| malformed_world_entry(SMSG_COMPRESSED_UPDATE_OBJECT, 0))?,
    ))
    .map_err(|_| malformed_world_entry(SMSG_COMPRESSED_UPDATE_OBJECT, 0))?;
    if !(4..=MAX_UPDATE_BODY_SIZE).contains(&declared_size) || compressed.is_empty() {
        return Err(malformed_world_entry(SMSG_COMPRESSED_UPDATE_OBJECT, 0));
    }

    let mut output = vec![0_u8; declared_size];
    let mut decompressor = Decompress::new(true);
    let Ok(status) = decompressor.decompress(compressed, &mut output, FlushDecompress::Finish)
    else {
        let consumed = usize::try_from(decompressor.total_in()).unwrap_or(compressed.len());
        return Err(malformed_world_entry(
            SMSG_COMPRESSED_UPDATE_OBJECT,
            4_usize.saturating_add(consumed),
        ));
    };
    let consumed = usize::try_from(decompressor.total_in())
        .map_err(|_| malformed_world_entry(SMSG_COMPRESSED_UPDATE_OBJECT, 4))?;
    let produced = usize::try_from(decompressor.total_out())
        .map_err(|_| malformed_world_entry(SMSG_COMPRESSED_UPDATE_OBJECT, 4 + consumed))?;
    if status != Status::StreamEnd || consumed != compressed.len() || produced != declared_size {
        return Err(malformed_world_entry(
            SMSG_COMPRESSED_UPDATE_OBJECT,
            4_usize.saturating_add(consumed),
        ));
    }
    Ok(output)
}

fn parse_update_body(
    body: &[u8],
    selected_guid: u64,
    opcode: u16,
) -> Result<Option<AuthoritativeSelfState>, ProtocolError> {
    if body.len() > MAX_UPDATE_BODY_SIZE {
        return Err(malformed_world_entry(opcode, 0));
    }
    let mut cursor = Cursor::new(body, opcode);
    let block_count = cursor.u32()?;
    if block_count > MAX_UPDATE_BLOCKS {
        return Err(cursor.malformed());
    }
    let mut found = None;

    for _ in 0..block_count {
        let update_type = cursor.u8()?;
        match update_type {
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
                    return Err(cursor.malformed());
                }
                if guid == selected_guid {
                    if found.is_some()
                        || update_type != UPDATE_TYPE_CREATE_OBJECT2
                        || object_type != OBJECT_TYPE_PLAYER
                        || movement.update_flags != REQUIRED_SELF_UPDATE_FLAGS
                    {
                        return Err(cursor.malformed());
                    }
                    let info = movement
                        .movement
                        .ok_or_else(|| cursor.malformed())?
                        .require_supported_self()?;
                    let speeds = movement.speeds.ok_or_else(|| cursor.malformed())?;
                    if speeds.run() <= 0.0 {
                        return Err(cursor.malformed());
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
                    return Err(cursor.malformed());
                }
                for _ in 0..count {
                    let _ = cursor.packed_guid()?;
                }
            }
            _ => return Err(cursor.malformed()),
        }
    }
    cursor.finish()?;
    Ok(found)
}

fn parse_remote_player_update_body(
    body: &[u8],
    opcode: u16,
) -> Result<Vec<RemotePlayerRecord>, ProtocolError> {
    if body.len() > MAX_UPDATE_BODY_SIZE {
        return Err(malformed_world_entry(opcode, 0));
    }
    let mut cursor = Cursor::new(body, opcode);
    let block_count = cursor.u32()?;
    if block_count > MAX_UPDATE_BLOCKS {
        return Err(cursor.malformed());
    }

    let mut records = Vec::new();
    for _ in 0..block_count {
        match cursor.u8()? {
            UPDATE_TYPE_VALUES => {
                let _ = cursor.packed_guid()?;
                consume_update_mask(&mut cursor)?;
            }
            UPDATE_TYPE_MOVEMENT => {
                let guid = cursor.packed_guid()?;
                let movement = parse_remote_movement_block(&mut cursor)?;
                if let Some(movement) = movement.movement
                    && let Some(record) = remote_movement_record(guid, movement, SMSG_UPDATE_OBJECT)
                {
                    records.push(record);
                }
            }
            update_type @ (UPDATE_TYPE_CREATE_OBJECT | UPDATE_TYPE_CREATE_OBJECT2) => {
                let guid = cursor.packed_guid()?;
                let object_type = cursor.u8()?;
                let movement = parse_remote_movement_block(&mut cursor)?;
                consume_update_mask(&mut cursor)?;
                if update_type == UPDATE_TYPE_CREATE_OBJECT2
                    && guid != 0
                    && object_type == OBJECT_TYPE_PLAYER
                    && movement.update_flags & UPDATE_FLAG_SELF == 0
                    && movement.update_flags
                        & (UPDATE_FLAG_LIVING | UPDATE_FLAG_STATIONARY_POSITION)
                        == UPDATE_FLAG_LIVING | UPDATE_FLAG_STATIONARY_POSITION
                    && let Some(movement) = movement.movement
                {
                    records.push(match movement {
                        RemoteMovementInfo::Usable(movement) => {
                            RemotePlayerRecord::PlayerCreate { guid, movement }
                        }
                        RemoteMovementInfo::Unusable(category) => {
                            RemotePlayerRecord::UnusableMovement { guid, category }
                        }
                    });
                }
            }
            UPDATE_TYPE_OUT_OF_RANGE => {
                let count = cursor.u32()?;
                if count > MAX_GUID_LIST {
                    return Err(cursor.malformed());
                }
                for _ in 0..count {
                    let guid = cursor.packed_guid()?;
                    if guid != 0 {
                        records.push(RemotePlayerRecord::OutOfRange { guid });
                    }
                }
            }
            UPDATE_TYPE_NEAR => {
                let count = cursor.u32()?;
                if count > MAX_GUID_LIST {
                    return Err(cursor.malformed());
                }
                for _ in 0..count {
                    let _ = cursor.packed_guid()?;
                }
            }
            _ => return Err(cursor.malformed()),
        }
    }
    cursor.finish()?;
    Ok(records)
}

#[derive(Clone, Copy)]
enum RemoteMovementInfo {
    Usable(AcoreMovementInfo),
    Unusable(RemotePlayerUnusableCategory),
}

struct RemoteMovementBlock {
    update_flags: u16,
    movement: Option<RemoteMovementInfo>,
}

fn remote_movement_record(
    guid: u64,
    movement: RemoteMovementInfo,
    opcode: u16,
) -> Option<RemotePlayerRecord> {
    (guid != 0).then_some(match movement {
        RemoteMovementInfo::Usable(movement) => RemotePlayerRecord::PlayerMovement {
            guid,
            movement,
            opcode,
        },
        RemoteMovementInfo::Unusable(category) => {
            RemotePlayerRecord::UnusableMovement { guid, category }
        }
    })
}

fn parse_remote_movement_block(
    cursor: &mut Cursor<'_>,
) -> Result<RemoteMovementBlock, ProtocolError> {
    let update_flags = cursor.u16()?;
    if update_flags & !UPDATE_FLAG_KNOWN != 0 {
        return Err(cursor.malformed());
    }

    let movement = if update_flags & UPDATE_FLAG_LIVING != 0 {
        let (movement, spline_enabled) = parse_remote_movement_info(cursor)?;
        let mut invalid_pose = false;
        for _ in 0..9 {
            invalid_pose |= !cursor.f32()?.is_finite();
        }
        if spline_enabled {
            consume_create_spline(cursor)?;
        }
        if invalid_pose {
            Some(RemoteMovementInfo::Unusable(
                RemotePlayerUnusableCategory::InvalidPose,
            ))
        } else {
            Some(movement)
        }
    } else {
        None
    };

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
    if update_flags & UPDATE_FLAG_LIVING == 0 {
        if update_flags & UPDATE_FLAG_POSITION != 0 {
            let _ = cursor.packed_guid()?;
            cursor.skip(8 * 4)?;
        } else if update_flags & UPDATE_FLAG_STATIONARY_POSITION != 0 {
            cursor.skip(4 * 4)?;
        }
    }

    Ok(RemoteMovementBlock {
        update_flags,
        movement,
    })
}

fn parse_remote_movement_info(
    cursor: &mut Cursor<'_>,
) -> Result<(RemoteMovementInfo, bool), ProtocolError> {
    let flags = cursor.u32()?;
    let flags2 = cursor.u16()?;
    let timestamp = cursor.u32()?;
    let mut invalid_pose = false;
    let position = remote_vector3(cursor, &mut invalid_pose)?;
    let orientation = remote_f32(cursor, &mut invalid_pose)?;

    let transport = if flags & MOVEMENT_FLAG_ON_TRANSPORT != 0 {
        let guid = cursor.packed_guid()?;
        let position = remote_vector3(cursor, &mut invalid_pose)?;
        let orientation = remote_f32(cursor, &mut invalid_pose)?;
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
    let pitch = has_pitch
        .then(|| remote_f32(cursor, &mut invalid_pose))
        .transpose()?;
    let fall_time_ms = cursor.u32()?;
    let jump = if flags & MOVEMENT_FLAG_FALLING != 0 {
        Some(AcoreJumpInfo {
            z_speed: remote_f32(cursor, &mut invalid_pose)?,
            sin_angle: remote_f32(cursor, &mut invalid_pose)?,
            cos_angle: remote_f32(cursor, &mut invalid_pose)?,
            xy_speed: remote_f32(cursor, &mut invalid_pose)?,
        })
    } else {
        None
    };
    let spline_elevation = (flags & MOVEMENT_FLAG_SPLINE_ELEVATION != 0)
        .then(|| remote_f32(cursor, &mut invalid_pose))
        .transpose()?;
    let movement = AcoreMovementInfo {
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
    };
    let remote_movement = if invalid_pose {
        RemoteMovementInfo::Unusable(RemotePlayerUnusableCategory::InvalidPose)
    } else if movement.is_ordinary_ground() {
        RemoteMovementInfo::Usable(movement)
    } else {
        RemoteMovementInfo::Unusable(RemotePlayerUnusableCategory::UnsupportedMovement)
    };
    Ok((remote_movement, flags & MOVEMENT_FLAG_SPLINE_ENABLED != 0))
}

fn remote_f32(cursor: &mut Cursor<'_>, invalid_pose: &mut bool) -> Result<f32, ProtocolError> {
    let value = cursor.f32()?;
    *invalid_pose |= !value.is_finite();
    Ok(value)
}

fn remote_vector3(
    cursor: &mut Cursor<'_>,
    invalid_pose: &mut bool,
) -> Result<[f32; 3], ProtocolError> {
    Ok([
        remote_f32(cursor, invalid_pose)?,
        remote_f32(cursor, invalid_pose)?,
        remote_f32(cursor, invalid_pose)?,
    ])
}

struct MovementBlock {
    update_flags: u16,
    movement: Option<AcoreMovementInfo>,
    speeds: Option<BootstrapSpeeds>,
}

fn parse_movement_block(cursor: &mut Cursor<'_>) -> Result<MovementBlock, ProtocolError> {
    let update_flags = cursor.u16()?;
    if update_flags & !UPDATE_FLAG_KNOWN != 0 {
        return Err(cursor.malformed());
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
        _ => return Err(cursor.malformed()),
    }
    cursor.skip(4 * 7)?;
    let node_count = cursor.u32()?;
    if node_count > MAX_SPLINE_NODES {
        return Err(cursor.malformed());
    }
    let node_bytes = usize::try_from(node_count)
        .ok()
        .and_then(|count| count.checked_mul(12))
        .ok_or_else(|| cursor.malformed())?;
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
            .ok_or_else(|| cursor.malformed())?;
    }
    let value_bytes = value_count
        .checked_mul(4)
        .ok_or_else(|| cursor.malformed())?;
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

const fn malformed_world_entry(opcode: u16, byte_offset: usize) -> ProtocolError {
    ProtocolError::MalformedWorldEntry {
        opcode,
        byte_offset,
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    opcode: u16,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8], opcode: u16) -> Self {
        Self {
            bytes,
            offset: 0,
            opcode,
        }
    }

    const fn malformed(&self) -> ProtocolError {
        malformed_world_entry(self.opcode, self.offset)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| self.malformed())?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| self.malformed())?;
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
            self.take(2)?.try_into().map_err(|_| self.malformed())?,
        ))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| self.malformed())?,
        ))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| self.malformed())?,
        ))
    }

    fn finite_f32(&mut self) -> Result<f32, ProtocolError> {
        let value = self.f32()?;
        value
            .is_finite()
            .then_some(value)
            .ok_or_else(|| self.malformed())
    }

    fn f32(&mut self) -> Result<f32, ProtocolError> {
        Ok(f32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| self.malformed())?,
        ))
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
            .ok_or_else(|| self.malformed())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{Compression, write::ZlibEncoder};

    use super::{
        AcoreJumpInfo, AcoreMovementInfo, Cursor, ForceMoveRoot, MOVEMENT_FLAG_FALLING,
        MOVEMENT_FLAG_ROOT, MSG_MOVE_HEARTBEAT_SERVER, MSG_MOVE_START_FORWARD_SERVER,
        MSG_MOVE_STOP_SERVER, OBJECT_TYPE_PLAYER, RemotePlayerRecord, RemotePlayerUnusableCategory,
        RemoteWorldTraceEvent, SMSG_COMPRESSED_UPDATE_OBJECT, SMSG_DESTROY_OBJECT,
        SMSG_UPDATE_OBJECT, UPDATE_FLAG_LIVING, UPDATE_FLAG_STATIONARY_POSITION,
        UPDATE_TYPE_CREATE_OBJECT, decode_force_move_root, decode_remote_player_frame,
        decode_remote_world_trace, encode_client_movement, encode_force_move_root_ack,
        push_packed_guid,
    };
    use crate::WorldServerFrame;

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

        let mut cursor = Cursor::new(&encoded, SMSG_UPDATE_OBJECT);
        assert_eq!(AcoreMovementInfo::decode(&mut cursor).unwrap(), expected);
        cursor.finish().unwrap();
    }

    #[test]
    fn client_ground_movement_prefixes_the_active_mover_packed_guid() {
        let body = encode_client_movement(
            0x0100_0000_0000_0007,
            AcoreMovementInfo::ground(42, [1.0, 2.0, 3.0], 0.5, true),
        )
        .unwrap();
        assert_eq!(&body[..3], &[0x81, 0x07, 0x01]);
        assert_eq!(u32::from_le_bytes(body[3..7].try_into().unwrap()), 1);
    }

    #[test]
    fn forced_root_round_trip_preserves_counter_and_marks_ack_movement_rooted() {
        let request = decode_force_move_root(&[0x01, 0x01, 0x78, 0x56, 0x34, 0x12]).unwrap();
        assert_eq!(
            request,
            ForceMoveRoot {
                guid: 1,
                counter: 0x1234_5678,
            }
        );

        let ack = encode_force_move_root_ack(
            request,
            AcoreMovementInfo::ground(42, [1.0, 2.0, 3.0], 0.5, false),
        )
        .unwrap();
        assert_eq!(&ack[..6], &[0x01, 0x01, 0x78, 0x56, 0x34, 0x12]);
        assert_eq!(
            u32::from_le_bytes(ack[6..10].try_into().unwrap()),
            MOVEMENT_FLAG_ROOT
        );
        assert_eq!(u32::from_le_bytes(ack[12..16].try_into().unwrap()), 42);
        assert_eq!(&ack[16..20], &1.0_f32.to_le_bytes());
        assert_eq!(&ack[20..24], &2.0_f32.to_le_bytes());
        assert_eq!(&ack[24..28], &3.0_f32.to_le_bytes());
        assert_eq!(&ack[28..32], &0.5_f32.to_le_bytes());
    }

    #[test]
    fn remote_trace_emits_only_semantic_player_lifecycle_records() {
        let guid = 0x0100_0000_0000_0002;
        let movement = AcoreMovementInfo::ground(42, [1.0, 2.0, 3.0], 0.5, true);
        let mut create = 1_u32.to_le_bytes().to_vec();
        create.push(3); // CreateObject2
        push_packed_guid(&mut create, guid);
        create.push(OBJECT_TYPE_PLAYER);
        create.extend_from_slice(
            &(UPDATE_FLAG_LIVING | UPDATE_FLAG_STATIONARY_POSITION).to_le_bytes(),
        );
        create.extend_from_slice(&movement.encode().unwrap());
        create.extend((0..9).flat_map(|_| 1.0_f32.to_le_bytes()));
        create.push(0); // no values-mask words

        assert_eq!(
            decode_remote_world_trace(SMSG_UPDATE_OBJECT, &create).unwrap(),
            vec![RemoteWorldTraceEvent::PlayerCreate { guid, movement }]
        );
        create[4] = UPDATE_TYPE_CREATE_OBJECT;
        assert!(
            decode_remote_world_trace(SMSG_UPDATE_OBJECT, &create)
                .unwrap()
                .is_empty()
        );

        let mut heartbeat = Vec::new();
        push_packed_guid(&mut heartbeat, guid);
        heartbeat.extend_from_slice(&movement.encode().unwrap());
        assert_eq!(
            decode_remote_world_trace(MSG_MOVE_HEARTBEAT_SERVER, &heartbeat).unwrap(),
            vec![RemoteWorldTraceEvent::Movement {
                guid,
                movement,
                opcode: MSG_MOVE_HEARTBEAT_SERVER,
            }]
        );

        let mut destroy = guid.to_le_bytes().to_vec();
        destroy.push(0);
        assert_eq!(
            decode_remote_world_trace(SMSG_DESTROY_OBJECT, &destroy).unwrap(),
            vec![RemoteWorldTraceEvent::Destroy { guid }]
        );
    }

    #[test]
    fn remote_player_decoder_emits_the_bounded_player_vocabulary() {
        let guid = 0x0100_0000_0000_0002;
        let movement = AcoreMovementInfo::ground(42, [1.0, 2.0, 3.0], 0.5, true);
        let create = remote_create_body(guid, OBJECT_TYPE_PLAYER, false, movement);
        assert_eq!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_UPDATE_OBJECT,
                create
            ))
            .unwrap(),
            vec![RemotePlayerRecord::PlayerCreate { guid, movement }]
        );

        let mut in_container = 1_u32.to_le_bytes().to_vec();
        in_container.push(1); // movement
        push_packed_guid(&mut in_container, guid);
        in_container.extend_from_slice(&UPDATE_FLAG_LIVING.to_le_bytes());
        in_container.extend_from_slice(&movement.encode().unwrap());
        in_container.extend((0..9).flat_map(|_| 1.0_f32.to_le_bytes()));
        assert_eq!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_UPDATE_OBJECT,
                in_container,
            ))
            .unwrap(),
            vec![RemotePlayerRecord::PlayerMovement {
                guid,
                movement,
                opcode: SMSG_UPDATE_OBJECT,
            }]
        );

        for opcode in [
            MSG_MOVE_START_FORWARD_SERVER,
            MSG_MOVE_HEARTBEAT_SERVER,
            MSG_MOVE_STOP_SERVER,
        ] {
            let mut movement_payload = Vec::new();
            push_packed_guid(&mut movement_payload, guid);
            movement_payload.extend_from_slice(&movement.encode().unwrap());
            assert_eq!(
                decode_remote_player_frame(&WorldServerFrame::test_complete(
                    opcode,
                    movement_payload,
                ))
                .unwrap(),
                vec![RemotePlayerRecord::PlayerMovement {
                    guid,
                    movement,
                    opcode,
                }]
            );
        }

        let mut out_of_range = 1_u32.to_le_bytes().to_vec();
        out_of_range.push(4);
        out_of_range.extend_from_slice(&1_u32.to_le_bytes());
        push_packed_guid(&mut out_of_range, guid);
        assert_eq!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_UPDATE_OBJECT,
                out_of_range,
            ))
            .unwrap(),
            vec![RemotePlayerRecord::OutOfRange { guid }]
        );

        let mut destroy = guid.to_le_bytes().to_vec();
        destroy.push(0);
        assert_eq!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_DESTROY_OBJECT,
                destroy
            ))
            .unwrap(),
            vec![RemotePlayerRecord::Destroy { guid }]
        );
    }

    #[test]
    fn remote_player_decoder_consumes_ignored_blocks_and_compression_exactly() {
        let guid = 0x0100_0000_0000_0002;
        let movement = AcoreMovementInfo::ground(42, [1.0, 2.0, 3.0], 0.5, false);
        let mut body = 5_u32.to_le_bytes().to_vec();
        body.push(0); // values
        push_packed_guid(&mut body, 99);
        body.push(0); // no mask words
        body.push(5); // near objects
        body.extend_from_slice(&1_u32.to_le_bytes());
        push_packed_guid(&mut body, 98);
        body.extend_from_slice(&remote_create_body(guid, 3, false, movement)[4..]); // NPC create
        body.extend_from_slice(&remote_create_body(guid, OBJECT_TYPE_PLAYER, true, movement)[4..]);
        body.extend_from_slice(&remote_create_body(guid, OBJECT_TYPE_PLAYER, false, movement)[4..]);
        let mut compressed = Vec::new();
        compressed.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&body).unwrap();
        compressed.extend_from_slice(&encoder.finish().unwrap());
        assert_eq!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_COMPRESSED_UPDATE_OBJECT,
                compressed,
            ))
            .unwrap(),
            vec![RemotePlayerRecord::PlayerCreate { guid, movement }]
        );
    }

    #[test]
    fn remote_player_decoder_redacts_unusable_movement_and_rejects_malformed_complete_frames() {
        let guid = 0x0100_0000_0000_0002;
        let mut unsupported = Vec::new();
        push_packed_guid(&mut unsupported, guid);
        let mut movement = AcoreMovementInfo::ground(42, [1.0, 2.0, 3.0], 0.5, false)
            .encode()
            .unwrap();
        movement[..4].copy_from_slice(&MOVEMENT_FLAG_FALLING.to_le_bytes());
        movement.extend_from_slice(&[0; 16]); // required jump payload
        unsupported.extend_from_slice(&movement);
        assert_eq!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                MSG_MOVE_HEARTBEAT_SERVER,
                unsupported,
            ))
            .unwrap(),
            vec![RemotePlayerRecord::UnusableMovement {
                guid,
                category: RemotePlayerUnusableCategory::UnsupportedMovement,
            }]
        );

        let mut invalid = Vec::new();
        push_packed_guid(&mut invalid, guid);
        invalid.extend_from_slice(
            &AcoreMovementInfo::ground(42, [1.0, 2.0, 3.0], 0.5, false)
                .encode()
                .unwrap(),
        );
        invalid[13..17].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                MSG_MOVE_HEARTBEAT_SERVER,
                invalid,
            ))
            .unwrap(),
            vec![RemotePlayerRecord::UnusableMovement {
                guid,
                category: RemotePlayerUnusableCategory::InvalidPose,
            }]
        );

        assert!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                MSG_MOVE_HEARTBEAT_SERVER,
                vec![0x80],
            ))
            .is_err()
        );
        assert!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_DESTROY_OBJECT,
                vec![0; 10],
            ))
            .is_err()
        );
        assert!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_COMPRESSED_UPDATE_OBJECT,
                vec![4, 0, 0, 0, 0xff],
            ))
            .is_err()
        );
        assert!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_UPDATE_OBJECT,
                [1_u32.to_le_bytes().as_slice(), &[0xff]].concat(),
            ))
            .is_err()
        );
        assert!(
            decode_remote_player_frame(&WorldServerFrame::test_complete(
                SMSG_UPDATE_OBJECT,
                [1_u32.to_le_bytes().as_slice(), &[0, 0x80]].concat(),
            ))
            .is_err()
        );
    }

    fn remote_create_body(
        guid: u64,
        object_type: u8,
        self_update: bool,
        movement: AcoreMovementInfo,
    ) -> Vec<u8> {
        let mut body = 1_u32.to_le_bytes().to_vec();
        body.push(3); // CreateObject2
        push_packed_guid(&mut body, guid);
        body.push(object_type);
        let flags = UPDATE_FLAG_LIVING | UPDATE_FLAG_STATIONARY_POSITION | u16::from(self_update);
        body.extend_from_slice(&flags.to_le_bytes());
        body.extend_from_slice(&movement.encode().unwrap());
        body.extend((0..9).flat_map(|_| 1.0_f32.to_le_bytes()));
        body.push(0); // no values-mask words
        body
    }
}
