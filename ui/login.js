const $ = (s) => document.querySelector(s);
const dialog = $('#login-dialog');
let timer = null;
$('#account-login').addEventListener('click', () => {
  $('#login-error').hidden = true;
  $('#device-code-box').hidden = true;
  $('#login-start').hidden = false;
  $('#login-start').disabled = false;
  $('#login-start').textContent = 'MIT MICROSOFT ANMELDEN';
  dialog.showModal();
});
$('#login-close').addEventListener('click', () => {
  dialog.close();
  if (timer) clearInterval(timer);
  timer = null;
});
$('#login-start').addEventListener('click', async () => {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) return fail('Der Login funktioniert nur in der Windows-App, nicht in der Browser-Vorschau.');
  const button = $('#login-start');
  button.disabled = true;
  try {
    const stored = JSON.parse(localStorage.getItem('rooks-client-state') || '{}');
    let clientId = stored.settings?.clientId?.trim();
    if (!clientId) {
      clientId = prompt('Füge Rooks eigene Microsoft-App-ID ein. Sie ist öffentlich und kein Passwort:');
      if (!clientId?.trim()) throw Error('Microsoft benötigt eine eigene App-ID für Rook.');
      stored.settings ||= {};
      stored.settings.clientId = clientId.trim();
      localStorage.setItem('rooks-client-state', JSON.stringify(stored));
    }
    const device = await invoke('begin_microsoft_login', { clientId: clientId.trim() });
    $('#device-code').textContent = device.user_code;
    const verificationLink = $('#verification-link');
    verificationLink.href = device.verification_uri;
    verificationLink.textContent = device.verification_uri;
    $('#device-code-box').hidden = false;
    $('#login-message').textContent = 'Melde dich mit deinem vorhandenen Microsoft-Konto an und gib dort den Code ein. Rook fragt niemals nach deinem Passwort.';
    await invoke('open_verification_page', { url: device.verification_uri });
    button.hidden = true;
    const deadline = Date.now() + device.expires_in * 1000;
    timer = setInterval(async () => {
      if (Date.now() >= deadline) { clearInterval(timer); timer = null; return fail('Der Code ist abgelaufen. Starte die Anmeldung erneut.'); }
      try {
        const account = await invoke('poll_microsoft_login');
        if (!account) return;
        clearInterval(timer); timer = null;
        $('#account-name').textContent = account.name;
        $('#account-subtitle').textContent = 'Minecraft Java Edition verbunden';
        $('#account-avatar').textContent = account.name.slice(0, 1).toUpperCase();
        $('#account-dot').style.background = '#85e0b1';
        window.dispatchEvent(new CustomEvent('rook-account-connected', { detail: account }));
        dialog.close();
        const toast = $('#toast'); toast.textContent = `Minecraft-Konto ${account.name} verbunden.`; toast.classList.add('show'); setTimeout(() => toast.classList.remove('show'), 2800);
      } catch (error) { clearInterval(timer); timer = null; fail(String(error)); }
    }, 2000);
  } catch (error) { fail(String(error)); }
});
function fail(message) {
  $('#login-error').textContent = message;
  $('#login-error').hidden = false;
  $('#login-message').textContent = 'Die Anmeldung konnte nicht abgeschlossen werden.';
  $('#login-start').hidden = false;
  $('#login-start').disabled = false;
  $('#login-start').textContent = 'ERNEUT VERSUCHEN';
}
