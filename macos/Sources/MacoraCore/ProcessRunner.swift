import Foundation

private final class ErrorBuffer: @unchecked Sendable { var data = Data() }

public enum ProcessRunner {
    public static func run(_ executable: String, _ arguments: [String], timeout: Double = 8, acceptedStatuses: Set<Int32> = [0]) async throws -> String {
        try await Task.detached(priority: .utility) {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: executable)
            process.arguments = arguments
            var env = ProcessInfo.processInfo.environment
            env["LC_ALL"] = "C"
            env["NO_COLOR"] = "1"
            process.environment = env
            process.standardInput = FileHandle.nullDevice
            // Separate pipes: a trailing "macora: ..." line on stderr must never be appended to JSON on stdout.
            let pipe = Pipe()
            let errPipe = Pipe()
            process.standardOutput = pipe
            process.standardError = errPipe
            try process.run()
            let timer = DispatchWorkItem { if process.isRunning { process.terminate() } }
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: timer)
            defer { timer.cancel() }
            let errBox = ErrorBuffer()
            let errDone = DispatchGroup()
            errDone.enter()
            DispatchQueue.global().async { errBox.data = errPipe.fileHandleForReading.readDataToEndOfFile(); errDone.leave() }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            errDone.wait()
            process.waitUntilExit()
            let output = String(decoding: data, as: UTF8.self)
            guard acceptedStatuses.contains(process.terminationStatus) else {
                let errText = String(decoding: errBox.data, as: UTF8.self)
                let detail = errText.isEmpty ? output : errText
                throw MacoraError.message(detail.isEmpty ? "Command failed or timed out: \(URL(fileURLWithPath: executable).lastPathComponent)" : detail)
            }
            return output
        }.value
    }
    public static func apply(_ plan: CommandPlan) async throws -> String {
        // The script is constant. User values are shell-quoted and passed through argv,
        // never interpolated into AppleScript source. macOS owns the password dialog.
        let script = "on run argv\nreturn do shell script (item 1 of argv) with administrator privileges\nend run"
        return try await run("/usr/bin/osascript", ["-e", script, plan.shellCommand], timeout: 120)
    }
}
