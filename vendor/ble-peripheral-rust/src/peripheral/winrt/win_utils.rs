use std::collections::HashMap;

use uuid::Uuid;
use windows::{
    Devices::Bluetooth::GenericAttributeProfile::{
        GattLocalCharacteristic, GattServiceProvider, GattSession,
    },
    Foundation::EventRegistrationToken,
    Storage::Streams::{DataReader, DataWriter, IBuffer, InMemoryRandomAccessStream},
    core::GUID,
};

pub struct GattCharacteristicObject {
    pub obj: GattLocalCharacteristic,
    pub subscribed_clients_token: EventRegistrationToken,
    pub read_requested_token: EventRegistrationToken,
    pub write_requested_token: EventRegistrationToken,
}

pub struct GattServiceProviderObject {
    pub obj: GattServiceProvider,
    pub advertisement_status_changed_token: EventRegistrationToken,
    pub characteristics: HashMap<Uuid, GattCharacteristicObject>,
}

pub(crate) fn to_guid(uuid: &Uuid) -> GUID {
    let (g1, g2, g3, g4) = uuid.as_fields();
    GUID::from_values(g1, g2, g3, g4.clone())
}

pub(crate) fn to_uuid(uuid: &GUID) -> Uuid {
    Uuid::from_fields(uuid.data1, uuid.data2, uuid.data3, &uuid.data4)
}

pub(crate) fn buffer_to_vec(buffer: &IBuffer) -> windows::core::Result<Vec<u8>> {
    let reader = DataReader::FromBuffer(buffer)?;
    let len = reader.UnconsumedBufferLength()? as usize;
    let mut data = vec![0u8; len];
    reader.ReadBytes(&mut data)?;
    Ok(data)
}

pub(crate) fn vec_to_buffer(vector: Vec<u8>) -> windows::core::Result<IBuffer> {
    let stream = InMemoryRandomAccessStream::new()?;
    let data_writer = DataWriter::CreateDataWriter(&stream)?;
    data_writer.WriteBytes(&vector)?;
    data_writer.DetachBuffer()
}

pub(crate) fn device_id_from_session(session: GattSession) -> windows::core::Result<String> {
    session
        .DeviceId()
        .and_then(|device| device.Id())
        .map(|id| id.to_string())
}
