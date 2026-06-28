//
//  ChatMessageView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct ChatMessageView: View {
    let entry: ChatEntry

    var body: some View {
        HStack(alignment: .top) {
            if entry.alignment == .trailing {
                Spacer(minLength: 52)
            }

            VStack(alignment: .leading, spacing: 7) {
                ChatEntryHeader(entry: entry)

                switch entry.kind {
                case .message:
                    Text(entry.body)
                        .font(.system(size: 15))
                        .lineSpacing(3)
                        .foregroundStyle(.primary)
                        .fixedSize(horizontal: false, vertical: true)

                case .streaming:
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        ProgressView()
                            .controlSize(.small)

                        Text(entry.body)
                            .font(.system(size: 15))
                            .lineSpacing(3)
                            .foregroundStyle(.primary)
                            .fixedSize(horizontal: false, vertical: true)
                    }

                case .toolResult:
                    ToolResultContent(entry: entry)

                case .command:
                    CommandContent(entry: entry)

                case .imageEvidence:
                    ImageEvidenceContent(entry: entry)

                case .approval:
                    ApprovalContent(entry: entry)

                case .requirements:
                    RequirementsContent(entry: entry)

                case .error:
                    ErrorContent(entry: entry)
                }
            }
            .padding(.horizontal, entry.kind.padding.horizontal)
            .padding(.vertical, entry.kind.padding.vertical)
            .frame(maxWidth: entry.alignment == .trailing ? 560 : 680, alignment: .leading)
            .background(entry.kind.surface)
            .overlay {
                RoundedRectangle(cornerRadius: entry.kind.radius, style: .continuous)
                    .stroke(entry.kind.stroke, lineWidth: entry.kind.strokeWidth)
            }
            .clipShape(RoundedRectangle(cornerRadius: entry.kind.radius, style: .continuous))

            if entry.alignment == .leading {
                Spacer(minLength: 52)
            }
        }
    }
}

private struct ChatEntryHeader: View {
    let entry: ChatEntry

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(entry.author)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(entry.kind.headerColor)

            if let detail = entry.detail {
                Text(detail)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer(minLength: 10)

            Text(entry.time)
                .font(.system(size: 11))
                .foregroundStyle(.tertiary)
        }
    }
}

private struct ToolResultContent: View {
    let entry: ChatEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(entry.body)
                .font(.system(size: 14))
                .foregroundStyle(.primary)
                .fixedSize(horizontal: false, vertical: true)

            Text(entry.output ?? "")
                .font(.system(size: 12, weight: .regular, design: .monospaced))
                .foregroundStyle(Color(red: 0.78, green: 0.86, blue: 0.92))
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.black.opacity(0.22))
                .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
        }
    }
}

private struct CommandContent: View {
    let entry: ChatEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text(entry.body)
                .font(.system(size: 14))
                .foregroundStyle(.primary)

            HStack(spacing: 8) {
                Image(systemName: "terminal")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)

                Text(entry.output ?? "")
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Color.primary.opacity(0.055))
            .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
        }
    }
}

private struct ImageEvidenceContent: View {
    let entry: ChatEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(entry.body)
                .font(.system(size: 14))
                .foregroundStyle(.primary)

            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(
                    LinearGradient(
                        colors: [
                            Color(red: 0.22, green: 0.27, blue: 0.32),
                            Color(red: 0.12, green: 0.16, blue: 0.21)
                        ],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .frame(height: 150)
                .overlay {
                    VStack(spacing: 8) {
                        Image(systemName: "photo")
                            .font(.system(size: 24, weight: .medium))
                            .foregroundStyle(.secondary)

                        Text("Image preview")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(.secondary)
                    }
                }
        }
    }
}

private struct ApprovalContent: View {
    let entry: ChatEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(entry.body)
                .font(.system(size: 14))
                .foregroundStyle(.primary)

            HStack(spacing: 8) {
                Button("Approve") {}
                    .buttonStyle(.borderedProminent)

                Button("Deny") {}
                    .buttonStyle(.bordered)
            }
            .controlSize(.small)
        }
    }
}

private struct RequirementsContent: View {
    let entry: ChatEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(entry.body)
                .font(.system(size: 14))
                .foregroundStyle(.primary)

            VStack(alignment: .leading, spacing: 6) {
                RequirementLine(text: "Screenshot evidence attached", complete: true)
                RequirementLine(text: "Reviewer response pending", complete: false)
            }
        }
    }
}

private struct RequirementLine: View {
    let text: String
    let complete: Bool

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: complete ? "checkmark.circle.fill" : "circle")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(complete ? Color(red: 0.47, green: 0.86, blue: 0.62) : .secondary)

            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
        }
    }
}

private struct ErrorContent: View {
    let entry: ChatEntry

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(Color(red: 1.00, green: 0.42, blue: 0.38))
                .padding(.top, 2)

            Text(entry.body)
                .font(.system(size: 14))
                .lineSpacing(2)
                .foregroundStyle(.primary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

struct ChatEntry: Identifiable {
    let id: String
    let author: String
    let detail: String?
    let time: String
    let body: String
    let output: String?
    let kind: ChatEntryKind
    let alignment: ChatEntryAlignment
}

enum ChatEntryAlignment {
    case leading
    case trailing
}

enum ChatEntryKind {
    case message
    case streaming
    case toolResult
    case command
    case imageEvidence
    case approval
    case requirements
    case error

    var radius: CGFloat {
        switch self {
        case .message, .streaming:
            return 16
        default:
            return 14
        }
    }

    var padding: (horizontal: CGFloat, vertical: CGFloat) {
        switch self {
        case .message, .streaming:
            return (15, 12)
        default:
            return (14, 13)
        }
    }

    var surface: Color {
        switch self {
        case .message:
            return Color.primary.opacity(0.070)
        case .streaming:
            return Color.primary.opacity(0.050)
        case .toolResult, .command, .imageEvidence, .requirements:
            return Color(red: 0.060, green: 0.080, blue: 0.105)
        case .approval:
            return Color(red: 0.145, green: 0.105, blue: 0.055)
        case .error:
            return Color(red: 0.145, green: 0.055, blue: 0.055)
        }
    }

    var stroke: Color {
        switch self {
        case .message, .streaming:
            return Color.primary.opacity(0.080)
        case .approval:
            return Color(red: 0.98, green: 0.67, blue: 0.25).opacity(0.28)
        case .error:
            return Color(red: 1.00, green: 0.42, blue: 0.38).opacity(0.30)
        default:
            return Color.primary.opacity(0.10)
        }
    }

    var strokeWidth: CGFloat {
        switch self {
        case .message, .streaming:
            return 0
        default:
            return 1
        }
    }

    var headerColor: Color {
        switch self {
        case .approval:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        case .error:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        case .toolResult, .command, .requirements, .imageEvidence:
            return Color(red: 0.47, green: 0.70, blue: 0.92)
        default:
            return .secondary
        }
    }
}

#Preview(traits: .landscapeLeft) {
    let entries = [
        ChatEntry(
            id: "user-message",
            author: "You",
            detail: nil,
            time: "10:12",
            body: "Clean up the release notes and attach proof before handing this back.",
            output: nil,
            kind: .message,
            alignment: .trailing
        ),
        ChatEntry(
            id: "assistant-message",
            author: "Runtime allow",
            detail: nil,
            time: "10:12",
            body: "I’ll review the current notes, make the smallest safe edit, then capture evidence for Requirements review.",
            output: nil,
            kind: .message,
            alignment: .leading
        ),
        ChatEntry(
            id: "streaming",
            author: "Runtime allow",
            detail: "responding",
            time: "10:13",
            body: "Checking the workspace state…",
            output: nil,
            kind: .streaming,
            alignment: .leading
        ),
        ChatEntry(
            id: "tool-result",
            author: "Code run",
            detail: "completed",
            time: "10:13",
            body: "Validated the edited files and collected the relevant output.",
            output: "2 files changed\nrequirements evidence ready",
            kind: .toolResult,
            alignment: .leading
        ),
        ChatEntry(
            id: "command",
            author: "Command",
            detail: "completed",
            time: "10:14",
            body: "Workspace status checked.",
            output: "git status --short",
            kind: .command,
            alignment: .leading
        ),
        ChatEntry(
            id: "image",
            author: "Evidence",
            detail: "preview available",
            time: "10:15",
            body: "Screenshot evidence was attached for review.",
            output: nil,
            kind: .imageEvidence,
            alignment: .leading
        ),
        ChatEntry(
            id: "approval",
            author: "Approval needed",
            detail: "owner action",
            time: "10:16",
            body: "This action needs approval before the runtime can continue.",
            output: nil,
            kind: .approval,
            alignment: .leading
        ),
        ChatEntry(
            id: "requirements",
            author: "Requirements",
            detail: "review active",
            time: "10:17",
            body: "The completion claim is waiting for reviewer confirmation.",
            output: nil,
            kind: .requirements,
            alignment: .leading
        ),
        ChatEntry(
            id: "error",
            author: "Runtime error",
            detail: nil,
            time: "10:18",
            body: "The runtime lost its stream connection. Reconnect before sending another message.",
            output: nil,
            kind: .error,
            alignment: .leading
        )
    ]

    ScrollView {
        LazyVStack(alignment: .leading, spacing: 14) {
            ForEach(entries) { entry in
                ChatMessageView(entry: entry)
            }
        }
        .padding(22)
    }
    .frame(width: 820, height: 760)
    .background(Color(red: 0.030, green: 0.040, blue: 0.056))
    .preferredColorScheme(.dark)
}
