// Validate the release secret without storing it in a file or command argument.
import CryptoKit
import Foundation

let input = FileHandle.standardInput.readDataToEndOfFile()
let encoded = String(data: input, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
guard CommandLine.arguments.count == 2,
      let seed = Data(base64Encoded: encoded), seed.count == 32,
      let expected = Data(base64Encoded: CommandLine.arguments[1]), expected.count == 32 else {
    fputs("Expected a 32-byte Sparkle signing seed and public key.\n", stderr)
    exit(1)
}
let key = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
guard key.publicKey.rawRepresentation == expected else {
    fputs("Release signing key does not match the public key embedded in Thor.\n", stderr)
    exit(1)
}
