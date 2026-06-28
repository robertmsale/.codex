//
//  RoleAuthorityEditorView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct RoleAuthorityEditorView: View {
    let actions: [RoleActionAuthority]
    @Binding var decisions: [String: RolePolicyDecision]
    @State private var selectedActionIds = Set<String>()

    private var groupedActions: [(String, [RoleActionAuthority])] {
        Dictionary(grouping: actions, by: { $0.groupLabel })
            .map { ($0.key, $0.value.sorted { $0.label < $1.label }) }
            .sorted { $0.0 < $1.0 }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .center, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Authority")
                        .font(.system(size: 17, weight: .semibold, design: .rounded))
                        .foregroundStyle(.primary)

                    Text("Set the policy decision for each runtime action.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Button("Select all") {
                    selectedActionIds = Set(actions.map(\.id))
                }
                .buttonStyle(.bordered)
                .controlSize(.small)

                Button("Clear selection") {
                    selectedActionIds.removeAll()
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(selectedActionIds.isEmpty)
            }

            if selectedActionIds.isEmpty {
                Text("Selection only controls bulk editing. The policy decision is changed separately on each row.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text("Changing one selected row applies that decision to \(selectedActionIds.count) selected actions.")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            VStack(alignment: .leading, spacing: 16) {
                ForEach(groupedActions, id: \.0) { groupLabel, groupActions in
                    VStack(alignment: .leading, spacing: 8) {
                        Text(groupLabel)
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(.tertiary)
                            .textCase(.uppercase)
                            .tracking(1.0)

                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(groupActions) { action in
                                RoleAuthorityRow(
                                    action: action,
                                    isSelected: selectedActionIds.contains(action.id),
                                    decision: Binding(
                                        get: { decisions[action.id, default: .off] },
                                        set: { newDecision in
                                            applyDecision(newDecision, from: action.id)
                                        }
                                    ),
                                    onToggleSelection: {
                                        toggleSelection(for: action.id)
                                    }
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    private func toggleSelection(for actionId: String) {
        if selectedActionIds.contains(actionId) {
            selectedActionIds.remove(actionId)
        } else {
            selectedActionIds.insert(actionId)
        }
    }

    private func applyDecision(_ decision: RolePolicyDecision, from actionId: String) {
        let affectedIds: Set<String>
        if selectedActionIds.contains(actionId), !selectedActionIds.isEmpty {
            affectedIds = selectedActionIds
        } else {
            affectedIds = [actionId]
        }

        for affectedId in affectedIds {
            decisions[affectedId] = decision
        }
    }
}

private struct RoleAuthorityRow: View {
    let action: RoleActionAuthority
    let isSelected: Bool
    @Binding var decision: RolePolicyDecision
    let onToggleSelection: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            Button {
                onToggleSelection()
            } label: {
                Image(systemName: isSelected ? "checkmark.square.fill" : "square")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(isSelected ? Color(red: 0.48, green: 0.68, blue: 1.00) : .secondary)
                    .frame(width: 24, height: 24)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Select \(action.label) \(action.id)")

            VStack(alignment: .leading, spacing: 4) {
                Text(action.label)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.primary)

                Text(action.id)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.middle)

                Text(action.descriptionText)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }

            Spacer(minLength: 12)

            Picker("Decision", selection: $decision) {
                ForEach(RolePolicyDecision.allCases) { option in
                    Text(option.title).tag(option)
                }
            }
            .labelsHidden()
            .pickerStyle(.menu)
            .frame(width: 180)
            .accessibilityLabel("Policy decision for \(action.label) \(action.id)")
        }
        .padding(12)
        .background(isSelected ? Color.primary.opacity(0.070) : Color.primary.opacity(0.030))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(isSelected ? Color.primary.opacity(0.20) : Color.primary.opacity(0.08), lineWidth: 1)
        }
    }
}

#Preview {
    let decisions: [String: RolePolicyDecision] = [
        "command.registry.preview": .ownerApproval,
        "command.registry.apply": .deny,
        "workflow.memory.read": .allow
    ]

    let actions = [
        RoleActionAuthority(id: "command.registry.preview", label: "Preview command registry change", groupLabel: "command.registry", descriptionText: "Validate and diff proposed command registry changes."),
        RoleActionAuthority(id: "command.registry.apply", label: "Apply command registry change", groupLabel: "command.registry", descriptionText: "Write an approved command definition into the registry."),
        RoleActionAuthority(id: "workflow.memory.read", label: "Read workflow memory", groupLabel: "workflow.memory", descriptionText: "Inspect persisted workflow memory for the current project."),
        RoleActionAuthority(id: "process.terminate", label: "Terminate process", groupLabel: "process", descriptionText: "Stop a running async process or service.")
    ]

    ScrollView {
        RoleAuthorityEditorView(actions: actions, decisions: .constant(decisions))
            .padding(20)
    }
    .frame(width: 620, height: 620)
    .preferredColorScheme(.dark)
}
