import Foundation

public protocol FissionNativeCapability {
    var name: String { get }
    func present(requestID: UInt64, payload: Data, completion: @escaping (Result<Data, Error>) -> Void)
}

public final class FissionNativeCapabilityRegistry {
    public static let shared = FissionNativeCapabilityRegistry()
    private var capabilities: [String: FissionNativeCapability] = [:]

    private init() {}

    public func register(_ capability: FissionNativeCapability) {
        capabilities[capability.name] = capability
    }

    public func present(name: String, requestID: UInt64, payload: Data, completion: @escaping (Result<Data, Error>) -> Void) -> Bool {
        guard let capability = capabilities[name] else {
            return false
        }
        capability.present(requestID: requestID, payload: payload, completion: completion)
        return true
    }
}
