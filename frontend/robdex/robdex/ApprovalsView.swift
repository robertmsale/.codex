//
//  ApprovalsView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct ApprovalsView: View {
    @State private var filter: ApprovalFilter = .actionable
    let approvals: [ApprovalItem]

    private var visibleApprovals: [ApprovalItem] {
        approvals.filter { filter.includes($0) }
    }

    var body: some View {
        VStack(spacing: 0) {
            ApprovalsHeader(filter: $filter)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    ForEach(visibleApprovals) { approval in
                        ApprovalReviewCard(approval: approval)
                    }
                }
                .padding(18)
                .frame(maxWidth: 820)
                .frame(maxWidth: .infinity)
            }
            .background(ApprovalsBackground())
        }
    }
}

private struct ApprovalsHeader: View {
    @Binding var filter: ApprovalFilter

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 14) {
            VStack(alignment: .leading, spacing: 3) {
                Text("Approvals")
                    .font(.system(size: 20, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Review blocked runtime actions before work continues.")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Menu {
                Picker("Filter", selection: $filter) {
                    ForEach(ApprovalFilter.allCases) { option in
                        Label(option.title, systemImage: option.icon)
                            .tag(option)
                    }
                }
            } label: {
                Image(systemName: "line.3.horizontal.decrease.circle")
            }
            .menuStyle(.button)
            .buttonStyle(.borderless)
            .help("Filter approvals")
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 16)
        .background(.regularMaterial)
        .overlay(alignment: .bottom) {
            Divider().opacity(0.6)
        }
    }
}

private struct ApprovalReviewCard: View {
    let approval: ApprovalItem
    @State private var reason = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 12) {
                ApprovalStateIcon(state: approval.state)
                    .padding(.top, 2)

                VStack(alignment: .leading, spacing: 5) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(approval.title)
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(.primary)
                            .lineLimit(2)

                        Spacer(minLength: 10)

                        Text(approval.state.label)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(approval.state.color)
                            .lineLimit(1)
                    }

                    Text(approval.summary)
                        .font(.system(size: 13))
                        .lineSpacing(2)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            ApprovalDetailGrid(approval: approval)

            switch approval.state {
            case .needsDecision:
                ApprovalDecisionControls(reason: $reason)

            case .approvedResume:
                HStack(spacing: 8) {
                    Button("Resume") {}
                        .buttonStyle(.borderedProminent)

                    Text("The action was approved and is waiting to continue.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
                .controlSize(.small)

            case .waitingForOwner:
                ApprovalUnavailableMessage(text: "Waiting for the required approver.")

            case .denied:
                ApprovalUnavailableMessage(text: "Denied actions cannot be resumed.")

            case .failed:
                ApprovalUnavailableMessage(text: "This approval failed before it could be decided.")

            case .unavailable:
                ApprovalUnavailableMessage(text: approval.unavailableReason ?? "This action is no longer available.")
            }
        }
        .padding(14)
        .background(approval.state.surface)
        .overlay {
            RoundedRectangle(cornerRadius: 15, style: .continuous)
                .stroke(approval.state.stroke, lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: 15, style: .continuous))
    }
}

private struct ApprovalDecisionControls: View {
    @Binding var reason: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            TextField("Reason for this decision", text: $reason, axis: .vertical)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .lineLimit(2...4)
                .padding(.horizontal, 11)
                .padding(.vertical, 9)
                .background(Color.primary.opacity(0.045))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.primary.opacity(0.10), lineWidth: 1)
                }

            HStack(spacing: 8) {
                Button("Approve") {}
                    .buttonStyle(.borderedProminent)

                Button("Deny") {}
                    .buttonStyle(.bordered)

                Spacer()

                Text("Reason required")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
            }
            .controlSize(.small)
        }
    }
}

private struct ApprovalDetailGrid: View {
    let approval: ApprovalItem

    var body: some View {
        VStack(spacing: 7) {
            ApprovalFactRow(label: "Requested by", value: approval.requestedBy)
            ApprovalFactRow(label: "Action", value: approval.action)
            ApprovalFactRow(label: "Scope", value: approval.scope)
        }
        .padding(.vertical, 2)
    }
}

private struct ApprovalFactRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(label)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.tertiary)
                .frame(width: 88, alignment: .leading)

            Text(value)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Spacer(minLength: 0)
        }
    }
}

private struct ApprovalUnavailableMessage: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.primary.opacity(0.035))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

private struct ApprovalStateIcon: View {
    let state: ApprovalState

    var body: some View {
        Image(systemName: state.icon)
            .font(.system(size: 17, weight: .semibold))
            .foregroundStyle(state.color)
            .frame(width: 22, height: 22)
    }
}

private struct ApprovalsBackground: View {
    var body: some View {
        ZStack {
            Color(red: 0.030, green: 0.040, blue: 0.056)

            LinearGradient(
                colors: [
                    Color.white.opacity(0.026),
                    Color.clear,
                    Color.black.opacity(0.10)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        }
    }
}

private enum ApprovalFilter: String, CaseIterable, Identifiable {
    case actionable
    case all

    var id: String { rawValue }

    var title: String {
        switch self {
        case .actionable:
            return "Actionable"
        case .all:
            return "Show hidden"
        }
    }

    var icon: String {
        switch self {
        case .actionable:
            return "hand.tap"
        case .all:
            return "tray.full"
        }
    }

    func includes(_ approval: ApprovalItem) -> Bool {
        switch self {
        case .actionable:
            return approval.state.isUserActionable
        case .all:
            return true
        }
    }
}

struct ApprovalItem: Identifiable {
    let id: String
    let title: String
    let summary: String
    let requestedBy: String
    let action: String
    let scope: String
    let state: ApprovalState
    let unavailableReason: String?
}

enum ApprovalState {
    case needsDecision
    case approvedResume
    case waitingForOwner
    case denied
    case failed
    case unavailable

    var label: String {
        switch self {
        case .needsDecision:
            return "Decision needed"
        case .approvedResume:
            return "Ready to resume"
        case .waitingForOwner:
            return "Waiting"
        case .denied:
            return "Denied"
        case .failed:
            return "Failed"
        case .unavailable:
            return "Unavailable"
        }
    }

    var icon: String {
        switch self {
        case .needsDecision:
            return "hand.raised.fill"
        case .approvedResume:
            return "play.circle.fill"
        case .waitingForOwner:
            return "clock.fill"
        case .denied:
            return "xmark.circle.fill"
        case .failed:
            return "exclamationmark.octagon.fill"
        case .unavailable:
            return "exclamationmark.triangle.fill"
        }
    }

    var color: Color {
        switch self {
        case .needsDecision, .approvedResume:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        case .waitingForOwner:
            return Color(red: 0.62, green: 0.70, blue: 0.80)
        case .denied, .failed, .unavailable:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        }
    }

    var surface: Color {
        switch self {
        case .needsDecision, .approvedResume:
            return Color(red: 0.120, green: 0.090, blue: 0.052)
        case .denied, .failed, .unavailable:
            return Color(red: 0.120, green: 0.050, blue: 0.052)
        default:
            return Color(red: 0.060, green: 0.080, blue: 0.105)
        }
    }

    var isUserActionable: Bool {
        switch self {
        case .needsDecision, .approvedResume:
            return true
        case .waitingForOwner, .denied, .failed, .unavailable:
            return false
        }
    }

    var stroke: Color {
        switch self {
        case .needsDecision, .approvedResume:
            return Color(red: 0.98, green: 0.67, blue: 0.25).opacity(0.30)
        case .denied, .failed, .unavailable:
            return Color(red: 1.00, green: 0.42, blue: 0.38).opacity(0.30)
        default:
            return Color.primary.opacity(0.10)
        }
    }
}

#Preview(traits: .landscapeLeft) {
    let approvals = [
        ApprovalItem(
            id: "needs-decision",
            title: "Allow file changes outside the selected workspace",
            summary: "The runtime paused before making a change that needs owner approval.",
            requestedBy: "Runtime allow",
            action: "Edit files",
            scope: "Project workspace boundary",
            state: .needsDecision,
            unavailableReason: nil
        ),
        ApprovalItem(
            id: "approved-resume",
            title: "Resume approved install check",
            summary: "The command was approved earlier but has not resumed yet.",
            requestedBy: "Runtime approval",
            action: "Run command",
            scope: "Current session",
            state: .approvedResume,
            unavailableReason: nil
        ),
        ApprovalItem(
            id: "waiting",
            title: "Request waiting for owner review",
            summary: "This approval requires a different approver before the session can continue.",
            requestedBy: "Requirements reviewer",
            action: "Continue blocked work",
            scope: "Selected session",
            state: .waitingForOwner,
            unavailableReason: nil
        ),
        ApprovalItem(
            id: "denied",
            title: "Network request denied",
            summary: "The requested network action was denied and the runtime must choose another path.",
            requestedBy: "Runtime no-rg",
            action: "Use network",
            scope: "Session policy",
            state: .denied,
            unavailableReason: nil
        ),
        ApprovalItem(
            id: "failed",
            title: "Approval could not be applied",
            summary: "The runtime could not match this approval to the current paused action.",
            requestedBy: "Runtime approval",
            action: "Resume action",
            scope: "Previous runtime state",
            state: .failed,
            unavailableReason: nil
        ),
        ApprovalItem(
            id: "unavailable",
            title: "Approval no longer matches current session state",
            summary: "The runtime moved on after rehydration, so this request is now read-only.",
            requestedBy: "Runtime allow",
            action: "Resume action",
            scope: "Previous runtime state",
            state: .unavailable,
            unavailableReason: "Refresh the session before deciding."
        )
    ]

    ApprovalsView(approvals: approvals)
        .frame(width: 820, height: 760)
        .preferredColorScheme(.dark)
}
