import Foundation

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
            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = pipe
            try process.run()
            let timer = DispatchWorkItem { if process.isRunning { process.terminate() } }
            DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: timer)
            defer { timer.cancel() }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            let output = String(decoding: data, as: UTF8.self)
            guard acceptedStatuses.contains(process.terminationStatus) else {
                throw MacoraError.message(output.isEmpty ? "Command failed or timed out: \(URL(fileURLWithPath: executable).lastPathComponent)" : output)
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
