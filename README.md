# Rook's Client

Ein eigenständig gestalteter Windows-Launcher-Prototyp mit Tauri 2, Rust und HTML/CSS/JavaScript. NoRisk-Quellcode, Marken und Grafiken wurden nicht übernommen.

## Starten

Die aktuelle Windows-App liegt unter `release\Rook's Client.exe`. Zum Installieren `release\RooksClient-Setup.msi` öffnen. Für einen neuen Build `.\build-windows.ps1` im Projektordner ausführen. Benötigt werden Rust stable mit MSVC und Visual Studio Build Tools mit **Desktopentwicklung mit C++**.

## Microsoft-/Minecraft-Anmeldung

Der Login nutzt Microsoft Device Code, tauscht danach über Xbox Live und XSTS gegen ein Minecraft Services-Token und lädt das Java-Profil. Das Passwort wird nie in Rook eingegeben oder gespeichert. Tokens werden derzeit nicht dauerhaft gespeichert; nach einem Neustart ist eine neue Anmeldung nötig. Ein erfolgreiches Konto-Login startet Minecraft noch nicht.

Für den ersten Login braucht Rook eine eigene Microsoft-App-ID. Eine fremde Launcher-ID darf nicht verwendet werden.

1. Im [Microsoft Entra Admin Center](https://entra.microsoft.com/) eine App-Registrierung für **persönliche Microsoft-Konten** erstellen. Microsoft beschreibt das unter [App registrieren](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app).
2. Unter **Authentication / Authentifizierung** die Option **Allow public client flows / Öffentliche Clientflows zulassen** aktivieren. Device Code Flow benötigt eine öffentliche Client-App ([Microsoft-Anleitung](https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-configuration)).
3. Die **Application (client) ID** aus der Übersicht kopieren. Das ist eine öffentliche Kennung, kein Geheimnis und kein Passwort.
4. Rook starten, links unten auf das Konto klicken und die Client-ID einfügen. Microsoft öffnet die offizielle Anmeldeseite im Browser; dort den angezeigten Einmalcode eingeben.

Ob Microsoft/Xbox die neu registrierte App für Minecraft Services akzeptiert, kann nur mit der registrierten App und einem berechtigten Java-Konto verifiziert werden. Es werden keine Login-Erfolge simuliert.

## Status

- Profile und Einstellungen werden lokal im WebView gespeichert.
- Mod-Suche lädt echte Treffer aus Modrinth; „Merken“ installiert keine Datei.
- Microsoft-/Xbox-/Minecraft-Profil-Anmeldung ist implementiert, benötigt aber die eigene App-ID.
- Minecraft-Installation und Spielstart, echte Mod-Installation, Cosmetics, Skin-Synchronisierung und Ingame-HUD sind noch nicht implementiert.
- Installer und EXE sind unsigniert; Windows kann beim Start deshalb einen SmartScreen-Hinweis anzeigen.

Die App-Oberfläche nutzt Tauri. Microsoft-Login erfolgt nativ in Rust. Authentifizierungs- und Profilinformationen werden nur für den laufenden Prozess verarbeitet; das Passwort wird nicht abgefragt.
