// P0 hardware-session helper, not a product control API.
// Uses the OS MIDI endpoint exposed after Audio MIDI Setup advertises the Mac.
// Listing never sends MIDI. Pulse requires an exact destination unique ID,
// known guitar binding, explicit values and a short press/release interval.
import Foundation
import CoreMIDI
import SystemConfiguration
import Darwin

struct ProbeFailure: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

func check(_ result: OSStatus, _ operation: String) throws {
    if result != noErr { throw ProbeFailure("\(operation): OSStatus \(result)") }
}

func stringProperty(_ object: MIDIObjectRef, _ key: CFString) -> String? {
    var value: Unmanaged<CFString>?
    guard MIDIObjectGetStringProperty(object, key, &value) == noErr else { return nil }
    return value?.takeRetainedValue() as String?
}

func integerProperty(_ object: MIDIObjectRef, _ key: CFString) -> Int32? {
    var value: Int32 = 0
    guard MIDIObjectGetIntegerProperty(object, key, &value) == noErr else { return nil }
    return value
}

func describe(_ endpoint: MIDIEndpointRef, _ kind: String) -> [String: Any] {
    var row: [String: Any] = ["kind": kind, "endpoint_ref": endpoint]
    if let value = stringProperty(endpoint, kMIDIPropertyName) { row["name"] = value }
    if let value = stringProperty(endpoint, kMIDIPropertyDisplayName) { row["display_name"] = value }
    if let value = stringProperty(endpoint, kMIDIPropertyDriverOwner) { row["driver_owner"] = value }
    if let value = integerProperty(endpoint, kMIDIPropertyUniqueID) { row["unique_id"] = value }
    if let value = integerProperty(endpoint, kMIDIPropertyOffline) { row["offline"] = value != 0 }
    return row
}

func emit(_ value: [String: Any]) throws {
    let data = try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
    print(String(decoding: data, as: UTF8.self))
    fflush(stdout)
}

func list() throws {
    var client = MIDIClientRef()
    try check(MIDIClientCreateWithBlock("Ringdown P0 inventory" as CFString, &client) { _ in }, "create inventory client")
    defer { MIDIClientDispose(client) }
    RunLoop.current.run(until: Date().addingTimeInterval(0.25))
    var endpoints: [[String: Any]] = []
    for index in 0..<MIDIGetNumberOfSources() {
        endpoints.append(describe(MIDIGetSource(index), "source"))
    }
    for index in 0..<MIDIGetNumberOfDestinations() {
        endpoints.append(describe(MIDIGetDestination(index), "destination"))
    }
    var devices: [[String: Any]] = []
    for index in 0..<MIDIGetNumberOfDevices() {
        let device = MIDIGetDevice(index)
        var row = describe(device, "device")
        var entities: [[String: Any]] = []
        for entityIndex in 0..<MIDIDeviceGetNumberOfEntities(device) {
            let entity = MIDIDeviceGetEntity(device, entityIndex)
            var entry = describe(entity, "entity")
            var destinations: [[String: Any]] = []
            for destinationIndex in 0..<MIDIEntityGetNumberOfDestinations(entity) {
                destinations.append(describe(MIDIEntityGetDestination(entity, destinationIndex), "destination"))
            }
            entry["destinations"] = destinations
            entities.append(entry)
        }
        row["entities"] = entities
        devices.append(row)
    }
    let computerName = SCDynamicStoreCopyComputerName(nil, nil) as String? ?? "unknown"
    try emit(["operation": "list", "timestamp": ISO8601DateFormatter().string(from: Date()),
              "computer_name": computerName, "os": ProcessInfo.processInfo.operatingSystemVersionString,
              "midi_client_created": true, "device_count": MIDIGetNumberOfDevices(),
              "endpoints": endpoints, "devices": devices])
}

func number(_ text: String, _ range: ClosedRange<Int>, _ label: String) throws -> Int {
    guard let value = Int(text), range.contains(value) else {
        throw ProbeFailure("\(label) must be in \(range)")
    }
    return value
}

func watch(_ seconds: Int) throws {
    var client = MIDIClientRef()
    try check(MIDIClientCreateWithBlock("Ringdown P0 connection watch" as CFString, &client) { _ in }, "create watch client")
    defer { MIDIClientDispose(client) }
    let end = Date().addingTimeInterval(Double(seconds))
    var previous: [MIDIEndpointRef]? = nil
    while Date() < end {
        RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        let endpoints = (0..<MIDIGetNumberOfSources()).map { MIDIGetSource($0) }
                      + (0..<MIDIGetNumberOfDestinations()).map { MIDIGetDestination($0) }
        if endpoints != previous {
            try list()
            previous = endpoints
        }
    }
}

func pulse(_ args: [String]) throws {
    guard args.count == 8, let uniqueID = Int32(args[0]) else {
        throw ProbeFailure("pulse requires UID cc|note CHANNEL NUMBER PRESS RELEASE MILLISECONDS ACTION_LABEL")
    }
    guard ["cc", "note"].contains(args[1]) else { throw ProbeFailure("type must be cc or note") }
    let channel = try number(args[2], 1...16, "channel")
    let control = try number(args[3], args[1] == "cc" ? 0...119 : 0...127, "number")
    let press = try number(args[4], 1...127, "press value")
    let release = try number(args[5], 0...127, "release value")
    let milliseconds = try number(args[6], 30...100, "press duration")
    guard ["effect-next", "effect-previous", "effect-switch", "beats-start-stop"].contains(args[7]) else {
        throw ProbeFailure("use a confirmed nondestructive binding; record/erase actions are excluded")
    }
    var client = MIDIClientRef()
    try check(MIDIClientCreateWithBlock("Ringdown P0 probe" as CFString, &client) { _ in }, "create client")
    defer { MIDIClientDispose(client) }
    RunLoop.current.run(until: Date().addingTimeInterval(0.25))
    var matches: [MIDIEndpointRef] = []
    for index in 0..<MIDIGetNumberOfDestinations() {
        let endpoint = MIDIGetDestination(index)
        if integerProperty(endpoint, kMIDIPropertyUniqueID) == uniqueID { matches.append(endpoint) }
    }
    guard matches.count == 1 else { throw ProbeFailure("expected one destination with UID \(uniqueID), found \(matches.count)") }
    let endpoint = matches[0]
    guard integerProperty(endpoint, kMIDIPropertyOffline) != 1 else { throw ProbeFailure("destination is offline") }
    var port = MIDIPortRef()
    try check(MIDIOutputPortCreate(client, "Ringdown P0 output" as CFString, &port), "create output")
    defer { MIDIPortDispose(port) }

    func send(_ status: UInt8, _ value: Int, _ phase: String) throws {
        let bytes: [UInt8] = [status, UInt8(control), UInt8(value)]
        var packets = MIDIPacketList()
        // CoreMIDI's C API requires pointers. One three-byte message fits in
        // the stack-owned MIDIPacketList; every pointer remains in scope.
        let result = withUnsafeMutablePointer(to: &packets) { pointer -> OSStatus in
            let first = MIDIPacketListInit(pointer)
            return bytes.withUnsafeBufferPointer { buffer in
                _ = MIDIPacketListAdd(pointer, MemoryLayout<MIDIPacketList>.size,
                                      first, 0, buffer.count, buffer.baseAddress!)
                return MIDISend(port, endpoint, pointer)
            }
        }
        try emit(["operation": "pulse", "phase": phase, "action_label": args[7],
                  "destination": describe(endpoint, "destination"), "bytes": bytes,
                  "host_uptime": ProcessInfo.processInfo.systemUptime,
                  "timestamp": ISO8601DateFormatter().string(from: Date()),
                  "os_status": result, "musical_result": "not_observed"])
        try check(result, "send \(phase)")
    }
    let status: UInt8 = args[1] == "cc" ? 0xB0 : 0x90
    try send(status | UInt8(channel - 1), press, "press")
    Thread.sleep(forTimeInterval: Double(milliseconds) / 1000)
    // One release attempt even if the peer disconnects; never retry a press.
    try send((args[1] == "cc" ? status : 0x80) | UInt8(channel - 1), release, "release")
}

do {
    let args = Array(CommandLine.arguments.dropFirst())
    if args.isEmpty || args == ["list"] {
        try list()
    } else if args.count == 2 && args[0] == "watch" {
        try watch(number(args[1], 1...600, "watch seconds"))
    } else if args.first == "pulse" {
        try pulse(Array(args.dropFirst()))
    } else {
        throw ProbeFailure("usage: midi_probe list | watch SECONDS | pulse UID cc|note CHANNEL NUMBER PRESS RELEASE MILLISECONDS ACTION_LABEL")
    }
} catch {
    fputs("\(error)\n", stderr)
    exit(1)
}
