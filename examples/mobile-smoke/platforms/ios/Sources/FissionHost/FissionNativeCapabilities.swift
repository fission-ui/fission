import Foundation
import FissionNativeModules

public enum FissionHostNativeCapabilities {
    public static func present(name: String, requestID: UInt64, payload: Data, completion: @escaping (Result<Data, Error>) -> Void) -> Bool {
        FissionNativeCapabilityRegistry.shared.present(name: name, requestID: requestID, payload: payload, completion: completion)
    }
}
