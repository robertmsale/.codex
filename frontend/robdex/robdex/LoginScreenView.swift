//
//  LoginScreen.swift
//  robdex
//
//  Created by Robert Sale on 6/27/26.
//

import SwiftUI

struct LoginScreen: View {
    @State private var manualAddress: String

    init(manualAddress: String = "") {
        _manualAddress = State(initialValue: manualAddress)
    }

    var body: some View {
        GeometryReader { proxy in
            let compact = proxy.size.width < 760

            ZStack {
                RuntimeLoginBackground()

                ScrollView {
                    if compact {
                        VStack(alignment: .leading, spacing: 28) {
                            LoginIntro()
                            LoginConnectionPanel(manualAddress: $manualAddress)
                        }
                        .padding(.horizontal, 22)
                        .padding(.vertical, 32)
                    } else {
                        HStack(alignment: .center, spacing: 52) {
                            LoginIntro()
                                .frame(maxWidth: 330, alignment: .leading)

                            LoginConnectionPanel(manualAddress: $manualAddress)
                                .frame(width: min(500, proxy.size.width * 0.48))
                        }
                        .frame(maxWidth: 980)
                        .padding(.horizontal, 48)
                        .padding(.vertical, 56)
                        .frame(minHeight: proxy.size.height)
                    }
                }
            }
        }
    }
}

private struct LoginIntro: View {
    #if os(macOS)
    let description = "Use a discovered service when available. Import a profile for a remote runtime, or enter an address manually."
    #else
    let description = "Import a profile for a remote runtime, or enter an address manually."
    #endif
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Agent Runtime")
                .font(.system(size: 13, weight: .semibold, design: .rounded))
                .foregroundStyle(.white)
                .textCase(.uppercase)
                .tracking(1.6)

            Text("Connect to your runtime.")
                .font(.system(size: 42, weight: .semibold, design: .rounded))
                .lineSpacing(-2)
                .foregroundStyle(.white)
                .fixedSize(horizontal: false, vertical: true)

            Text(description)
                .font(.system(size: 16, weight: .regular))
                .lineSpacing(3)
                .foregroundStyle(.white)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct LoginConnectionPanel: View {
    @Binding var manualAddress: String

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Runtime setup")
                    .font(.system(size: 20, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Nothing is connected yet.")
                    .font(.system(size: 14))
                    .foregroundStyle(.secondary)
            }
            .padding(.bottom, 22)
            #if os(macOS)
            ConnectionOptionRow(
                title: "Local runtime",
                status: "Not checked",
                actionTitle: "Refresh",
                secondaryActionTitle: "Connect",
                secondaryDisabled: true
            )

            Divider()
                .opacity(0.55)
            
            #endif

            ConnectionOptionRow(
                title: "Remote profile",
                status: "No profile imported",
                actionTitle: "Import profile",
                secondaryActionTitle: "Connect",
                secondaryDisabled: true
            )

            Divider()
                .opacity(0.55)
            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Manual address")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(.primary)

                    Text("Use this when discovery is unavailable.")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }

                HStack(spacing: 10) {
                    TextField("Runtime address", text: $manualAddress)
                        .textFieldStyle(.plain)
                        .font(.system(size: 14, weight: .medium, design: .monospaced))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 11)
                        .background(
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .fill(Color.primary.opacity(0.055))
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .stroke(Color.primary.opacity(0.12), lineWidth: 1)
                        )

                    Button("Connect") {}
                        .buttonStyle(.borderedProminent)
                        .controlSize(.large)
                }
            }
            .padding(.top, 22)
        }
        .padding(24)
        .background(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .fill(.regularMaterial)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(Color.primary.opacity(0.10), lineWidth: 1)
        )
    }
}

private struct ConnectionOptionRow: View {
    let title: String
    let status: String
    let actionTitle: String
    let secondaryActionTitle: String
    let secondaryDisabled: Bool

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(.primary)

                Text(status)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            }

            Spacer(minLength: 16)

            HStack(spacing: 8) {
                Button(actionTitle) {}
                    .buttonStyle(.bordered)

                Button(secondaryActionTitle) {}
                    .buttonStyle(.borderedProminent)
                    .disabled(secondaryDisabled)
            }
        }
        .padding(.vertical, 18)
    }
}

private struct RuntimeLoginBackground: View {
    var body: some View {
        LinearGradient(
            colors: [
                Color(red: 0.035, green: 0.055, blue: 0.085),
                Color(red: 0.055, green: 0.075, blue: 0.105),
                Color(red: 0.025, green: 0.035, blue: 0.055)
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
        .ignoresSafeArea()
        .overlay(alignment: .topTrailing) {
            Circle()
                .fill(Color.orange.opacity(0.10))
                .frame(width: 420, height: 420)
                .blur(radius: 90)
                .offset(x: 140, y: -160)
        }
        .overlay(alignment: .bottomLeading) {
            Circle()
                .fill(Color.blue.opacity(0.08))
                .frame(width: 360, height: 360)
                .blur(radius: 100)
                .offset(x: -120, y: 120)
        }
    }
}

#Preview(traits: .landscapeLeft) {
    LoginScreen(manualAddress: "http://127.0.0.1:8765")
}
