//
//  RoleVersionListView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct RoleVersionListView: View {
    let role: RuntimeRole?
    @Binding var selectedVersionId: String
    let onActivateVersion: (String) -> Void
    let onExportVersion: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Versions")
                .font(.system(size: 17, weight: .semibold, design: .rounded))
                .foregroundStyle(.primary)

            if let role, !role.versions.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(role.versions) { version in
                        RoleVersionRow(
                            version: version,
                            isSelected: version.id == selectedVersionId,
                            onSelect: {
                                selectedVersionId = version.id
                            },
                            onActivate: {
                                onActivateVersion(version.id)
                            },
                            onExport: {
                                onExportVersion(version.id)
                            }
                        )
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    Text("No versions")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(.primary)

                    Text("Save a role draft to create an immutable version.")
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
    }
}

private struct RoleVersionRow: View {
    let version: RuntimeRoleVersion
    let isSelected: Bool
    let onSelect: () -> Void
    let onActivate: () -> Void
    let onExport: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(version.label)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(.primary)

                Spacer(minLength: 8)

                Text(version.isActive ? "Active" : "Saved")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(version.isActive ? Color.green : .secondary)
            }

            Text(version.createdText)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)

            Text(version.summary)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .lineLimit(2)

            HStack(spacing: 8) {
                Button("Activate") {
                    onActivate()
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(version.isActive)
                .help(version.isActive ? "This version is already active." : "Make this immutable version active.")

                Button("Export") {
                    onExport()
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isSelected ? Color.primary.opacity(0.085) : Color.primary.opacity(0.030))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(isSelected ? Color.primary.opacity(0.22) : Color.primary.opacity(0.08), lineWidth: 1)
        }
        .contentShape(Rectangle())
        .onTapGesture {
            onSelect()
        }
    }
}

#Preview {
    let role = RuntimeRole(
        id: "runtime-allow",
        label: "Runtime allow",
        descriptionText: "General runtime role for normal project work.",
        activeVersionLabel: "v4 active",
        isArchived: false,
        versions: [
            RuntimeRoleVersion(id: "role-version-4", label: "v4", summary: "Tightened command registry authority.", createdText: "Today, 9:40 AM", isActive: true),
            RuntimeRoleVersion(id: "role-version-3", label: "v3", summary: "Added workflow memory read authority.", createdText: "Yesterday, 4:12 PM", isActive: false)
        ]
    )

    RoleVersionListView(
        role: role,
        selectedVersionId: .constant("role-version-4"),
        onActivateVersion: { _ in },
        onExportVersion: { _ in }
    )
    .padding(20)
    .frame(width: 320, height: 420)
    .preferredColorScheme(.dark)
}
