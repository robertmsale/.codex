//
//  RoleListView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct RoleListView: View {
    let roles: [RuntimeRole]
    @Binding var selectedRoleId: String
    let onCreateRole: () -> Void
    let onArchiveRole: (String) -> Void
    let onUnarchiveRole: (String) -> Void

    private var activeRoles: [RuntimeRole] {
        roles.filter { !$0.isArchived }
    }

    private var archivedRoles: [RuntimeRole] {
        roles.filter { $0.isArchived }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .center, spacing: 12) {
                Text("Roles")
                    .font(.system(size: 17, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Spacer()

                Button {
                    onCreateRole()
                } label: {
                    Image(systemName: "plus")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Create role")
            }

            if activeRoles.isEmpty && archivedRoles.isEmpty {
                RoleListEmptyState()
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 8) {
                        ForEach(activeRoles) { role in
                            RoleRow(
                                role: role,
                                isSelected: role.id == selectedRoleId,
                                onSelect: {
                                    selectedRoleId = role.id
                                },
                                onArchive: {
                                    onArchiveRole(role.id)
                                },
                                onUnarchive: {
                                    onUnarchiveRole(role.id)
                                }
                            )
                        }

                        if !archivedRoles.isEmpty {
                            Text("Archived")
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(.tertiary)
                                .textCase(.uppercase)
                                .tracking(1.0)
                                .padding(.top, activeRoles.isEmpty ? 0 : 10)

                            ForEach(archivedRoles) { role in
                                RoleRow(
                                    role: role,
                                    isSelected: role.id == selectedRoleId,
                                    onSelect: {
                                        selectedRoleId = role.id
                                    },
                                    onArchive: {
                                        onArchiveRole(role.id)
                                    },
                                    onUnarchive: {
                                        onUnarchiveRole(role.id)
                                    }
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

private struct RoleRow: View {
    let role: RuntimeRole
    let isSelected: Bool
    let onSelect: () -> Void
    let onArchive: () -> Void
    let onUnarchive: () -> Void

    var body: some View {
        Button {
            onSelect()
        } label: {
            VStack(alignment: .leading, spacing: 7) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(role.label)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(.primary)
                        .lineLimit(1)

                    Spacer(minLength: 8)

                    Text(role.isArchived ? "Archived" : role.activeVersionLabel)
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(role.isArchived ? .tertiary : .secondary)
                        .lineLimit(1)
                }

                Text(role.id)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.middle)

                Text(role.descriptionText)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(isSelected ? Color.primary.opacity(0.085) : Color.primary.opacity(0.030))
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(isSelected ? Color.primary.opacity(0.22) : Color.primary.opacity(0.08), lineWidth: 1)
            }
        }
        .buttonStyle(.plain)
        .contextMenu {
            if role.isArchived {
                Button("Unarchive role") {
                    onUnarchive()
                }
            } else {
                Button("Archive role", role: .destructive) {
                    onArchive()
                }
            }
        }
    }
}

private struct RoleListEmptyState: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("No roles")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(.primary)

            Text("Create a role before assigning project or session defaults.")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.primary.opacity(0.035))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(Color.primary.opacity(0.08), lineWidth: 1)
        }
    }
}

#Preview {
    let roles = [
        RuntimeRole(
            id: "runtime-allow",
            label: "Runtime allow",
            descriptionText: "General runtime role for normal project work.",
            activeVersionLabel: "v4 active",
            isArchived: false,
            versions: []
        ),
        RuntimeRole(
            id: "requirements-reviewer",
            label: "Requirements reviewer",
            descriptionText: "Reviews requirement claims and image evidence.",
            activeVersionLabel: "v2 active",
            isArchived: false,
            versions: []
        ),
        RuntimeRole(
            id: "legacy-debugger",
            label: "Legacy debugger",
            descriptionText: "Old debugger role retained for audit history.",
            activeVersionLabel: "v1 active",
            isArchived: true,
            versions: []
        )
    ]

    RoleListView(
        roles: roles,
        selectedRoleId: .constant("runtime-allow"),
        onCreateRole: {},
        onArchiveRole: { _ in },
        onUnarchiveRole: { _ in }
    )
    .padding(20)
    .frame(width: 320, height: 520)
    .preferredColorScheme(.dark)
}
