// P0 diagnostic peripheral. No advertising or MIDI transmission on startup.
// Interface facts: https://docs.silabs.com/bluetooth/2.13/bluetooth-code-examples-applications/midi-over-ble
// Notifications: https://developer.apple.com/documentation/corebluetooth/cbperipheralmanager
import Foundation
import CoreBluetooth
import Darwin

let arguments = Array(CommandLine.arguments.dropFirst())
let unencryptedDiagnostic = arguments.contains("--unencrypted-diagnostic")
let paths = arguments.filter { $0 != "--unencrypted-diagnostic" }
guard paths.count <= 1 else { fatalError("usage: ringdown-ble-midi [logfile] [--unencrypted-diagnostic]") }
let logFile: FileHandle? = {
    guard let path = paths.first else { return nil }
    if !FileManager.default.fileExists(atPath: path) { FileManager.default.createFile(atPath: path, contents: nil) }
    guard let handle = FileHandle(forWritingAtPath: path) else { return nil }
    handle.seekToEndOfFile()
    return handle
}()

func record(_ event: String, _ details: [String: Any] = [:]) {
    var value = details
    value["event"] = event
    value["timestamp"] = ISO8601DateFormatter().string(from: Date())
    guard let json = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) else { return }
    var line = json
    line.append(10)
    FileHandle.standardOutput.write(line)
    logFile?.write(line)
}

final class Pedal: NSObject, CBPeripheralManagerDelegate {
    let serviceID = CBUUID(string: "03B80E5A-EDE8-4B33-A751-6CE34EC4C700")
    let ioID = CBUUID(string: "7772E5DB-3868-4112-A1A9-F2669D106BF3")
    var manager: CBPeripheralManager!
    var io: CBMutableCharacteristic?
    var serviceReady = false
    var subscribed: [UUID: CBCentral] = [:]
    var pulsePending = false
    var subscriptionGeneration: UInt64 = 0

    override init() {
        super.init()
        manager = CBPeripheralManager(delegate: self, queue: .main,
                                      options: [CBPeripheralManagerOptionShowPowerAlertKey: false])
    }

    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        record("manager_state", ["state": peripheral.state.rawValue,
                                 "authorization": CBManager.authorization.rawValue])
        guard peripheral.state == .poweredOn else {
            serviceReady = false
            subscribed.removeAll()
            subscriptionGeneration &+= 1
            return
        }
        // Standards-oriented default; opt-in compatibility diagnosis changes only access requirements.
        let properties: CBCharacteristicProperties = unencryptedDiagnostic
            ? [.read, .writeWithoutResponse, .notify]
            : [.read, .writeWithoutResponse, .notify, .notifyEncryptionRequired]
        let permissions: CBAttributePermissions = unencryptedDiagnostic
            ? [.readable, .writeable] : [.readEncryptionRequired, .writeEncryptionRequired]
        let characteristic = CBMutableCharacteristic(type: ioID,
            properties: properties, value: nil, permissions: permissions)
        let service = CBMutableService(type: serviceID, primary: true)
        service.characteristics = [characteristic]
        io = characteristic
        peripheral.add(service)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        serviceReady = error == nil
        record("service_registered", ["ready": serviceReady, "error": error?.localizedDescription ?? "none"])
    }

    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        record("advertising_result", ["advertising": peripheral.isAdvertising,
                                      "error": error?.localizedDescription ?? "none"])
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral,
                           didSubscribeTo characteristic: CBCharacteristic) {
        subscriptionGeneration &+= 1
        subscribed[central.identifier] = central
        peripheral.stopAdvertising()
        record("subscribed", ["central": central.identifier.uuidString,
                               "maximum_notification_bytes": central.maximumUpdateValueLength,
                               "subscriber_count": subscribed.count])
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral,
                           didUnsubscribeFrom characteristic: CBCharacteristic) {
        subscriptionGeneration &+= 1
        subscribed.removeValue(forKey: central.identifier)
        record("unsubscribed", ["central": central.identifier.uuidString])
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveRead request: CBATTRequest) {
        record("read_request", ["central": request.central.identifier.uuidString, "offset": request.offset])
        guard request.characteristic.uuid == ioID, request.offset == 0 else {
            peripheral.respond(to: request, withResult: .invalidOffset)
            return
        }
        request.value = Data()
        peripheral.respond(to: request, withResult: .success)
    }

    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        for request in requests {
            record("write_request", ["central": request.central.identifier.uuidString,
                                      "bytes": Array(request.value ?? Data())])
        }
    }

    func send(_ value: UInt8, _ central: CBCentral, _ phase: String) -> Bool {
        guard manager.state == .poweredOn, let characteristic = io,
              subscribed[central.identifier] != nil, central.maximumUpdateValueLength >= 5 else {
            record("send_refused", ["phase": phase, "reason": "original subscriber unavailable"])
            return false
        }
        let millis = UInt64(ProcessInfo.processInfo.systemUptime * 1000) & 0x1FFF
        let packet: [UInt8] = [0x80 | UInt8(millis >> 7), 0x80 | UInt8(millis & 0x7F), 0xB0, 67, value]
        let accepted = manager.updateValue(Data(packet), for: characteristic, onSubscribedCentrals: [central])
        record("midi_notification", ["phase": phase, "bytes": packet, "accepted_by_os": accepted,
                                      "central": central.identifier.uuidString, "musical_result": "not_observed"])
        return accepted
    }

    func command(_ line: String) {
        switch line.trimmingCharacters(in: .whitespacesAndNewlines) {
        case "status":
            record("status", ["service_ready": serviceReady, "advertising": manager.isAdvertising,
                               "subscriber_count": subscribed.count])
        case "advertise":
            guard serviceReady, subscribed.isEmpty else {
                record("advertise_refused", ["reason": "service unavailable or already subscribed"])
                return
            }
            // Keep the name and 128-bit service within the initial payload;
            // relying on Apple's service-UUID overflow area would hide it
            // from a non-Apple central such as the guitar.
            manager.startAdvertising([CBAdvertisementDataLocalNameKey: "Ringdown",
                                       CBAdvertisementDataServiceUUIDsKey: [serviceID]])
        case "pulse67":
            guard !pulsePending, subscribed.count == 1, let central = subscribed.values.first else {
                record("pulse_refused", ["reason": "need exactly one subscriber and no pending pulse"])
                return
            }
            let generation = subscriptionGeneration
            guard send(127, central, "press") else { return }
            pulsePending = true
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(50)) {
                defer { self.pulsePending = false }
                guard self.subscriptionGeneration == generation else {
                    record("send_refused", ["phase": "release", "reason": "subscription changed; no reconnect replay"])
                    return
                }
                _ = self.send(0, central, "release")
            }
        case "stop":
            manager.stopAdvertising()
            record("advertising_stopped")
        case "quit":
            guard !pulsePending else { record("quit_refused", ["reason": "release pending"]); return }
            manager.stopAdvertising()
            manager.removeAllServices()
            record("exit")
            exit(0)
        default:
            record("command_refused", ["commands": "status advertise pulse67 stop quit"])
        }
    }
}

let pedal = Pedal()
var input = ""
let console = DispatchSource.makeReadSource(fileDescriptor: STDIN_FILENO, queue: .main)
console.setEventHandler {
    var bytes = [UInt8](repeating: 0, count: 1024)
    let count = read(STDIN_FILENO, &bytes, bytes.count)
    guard count > 0 else { console.cancel(); return }
    input += String(decoding: bytes.prefix(count), as: UTF8.self)
    while let newline = input.firstIndex(of: "\n") {
        let line = String(input[..<newline])
        input.removeSubrange(...newline)
        pedal.command(line)
    }
}
console.resume()
record("started", ["advertising": false, "commands": "status advertise pulse67 stop quit",
                   "access_mode": unencryptedDiagnostic ? "unencrypted_diagnostic" : "encryption_required"])
RunLoop.main.run()
