<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import {
    appConfig, serverRunning,
    upsertServerProfile, deleteServerProfile, setActiveServerProfile,
    type ServerProfile,
  } from '../lib/store'

  let dropdownOpen = false
  let mode: 'list' | 'form' = 'list'
  let editingId: string | null = null
  let formName = ''
  let formExe = ''
  let error = ''
  let deletingId: string | null = null

  $: profiles = $appConfig.server_profiles ?? []
  $: activeProfile = profiles.find(p => p.id === $appConfig.active_server_profile_id) ?? null

  // Fallback label when the legacy single server_exe is set but no profiles
  // have been created yet -- keeps existing installs working unchanged.
  $: fallbackLabel = $appConfig.server_exe
    ? ($appConfig.server_exe.split(/[\\/]/).slice(-2, -1)[0] || 'Server')
    : 'No server set'

  $: currentLabel = activeProfile?.name ?? fallbackLabel

  function toggleDropdown() {
    dropdownOpen = !dropdownOpen
    if (!dropdownOpen) resetForm()
  }

  function closeDropdown() {
    dropdownOpen = false
    resetForm()
  }

  function resetForm() {
    mode = 'list'
    editingId = null
    formName = ''
    formExe = ''
    error = ''
    deletingId = null
  }

  function startAdd() {
    mode = 'form'
    editingId = null
    formName = ''
    formExe = ''
    error = ''
  }

  function startEdit(p: ServerProfile) {
    mode = 'form'
    editingId = p.id
    formName = p.name
    formExe = p.server_exe
    error = ''
  }

  async function browseExe() {
    const selected = await open({
      filters: [{ name: 'Executable', extensions: ['exe'] }],
      multiple: false,
    })
    if (selected && typeof selected === 'string') {
      formExe = selected
      if (!formName.trim()) {
        formName = selected.split(/[\\/]/).slice(-2, -1)[0] || ''
      }
    }
  }

  async function saveForm() {
    const name = formName.trim()
    const server_exe = formExe.trim()
    if (!name) { error = 'Name is required.'; return }
    if (!server_exe) { error = 'Server executable is required.'; return }

    try {
      await upsertServerProfile({
        id: editingId ?? crypto.randomUUID(),
        name,
        server_exe,
      })
      mode = 'list'
      editingId = null
      error = ''
    } catch (e) {
      error = String(e)
    }
  }

  async function switchTo(profileId: string) {
    if (profileId === $appConfig.active_server_profile_id) return
    error = ''
    try {
      await setActiveServerProfile(profileId)
      closeDropdown()
    } catch (e) {
      error = String(e)
    }
  }

  async function confirmDelete(profileId: string) {
    try {
      await deleteServerProfile(profileId)
      deletingId = null
    } catch (e) {
      error = String(e)
    }
  }

  function onWindowClick(e: MouseEvent) {
    if (!dropdownOpen) return
    const target = e.target as HTMLElement
    if (!target.closest('.profile-switcher')) closeDropdown()
  }
</script>

<svelte:window on:click={onWindowClick} />

<div class="profile-switcher">
  <button class="switcher-btn" on:click|stopPropagation={toggleDropdown} title={activeProfile?.server_exe ?? $appConfig.server_exe}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <rect x="2" y="3" width="20" height="18" rx="2"/>
      <line x1="2" y1="9" x2="22" y2="9"/>
    </svg>
    <span class="switcher-label">{currentLabel}</span>
    {#if $serverRunning}
      <span class="running-dot" title="Server running"></span>
    {/if}
    <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <polyline points="6 9 12 15 18 9"/>
    </svg>
  </button>

  {#if dropdownOpen}
    <div class="dropdown" role="presentation" on:click|stopPropagation>
      {#if mode === 'list'}
        {#if profiles.length === 0}
          <div class="empty-note">
            No server profiles yet.{#if $appConfig.server_exe} Currently using the legacy single server path from Settings.{/if}
          </div>
        {:else}
          <div class="profile-list">
            {#each profiles as p (p.id)}
              <div class="profile-row" class:active={p.id === $appConfig.active_server_profile_id}>
                <button class="profile-select" on:click={() => switchTo(p.id)} title={p.server_exe}>
                  <span class="profile-dot" class:on={p.id === $appConfig.active_server_profile_id}></span>
                  <span class="profile-name">{p.name}</span>
                </button>
                {#if deletingId === p.id}
                  <button class="icon-btn confirm" title="Confirm delete" on:click={() => confirmDelete(p.id)}>Delete?</button>
                  <button class="icon-btn" title="Cancel" on:click={() => deletingId = null}>✕</button>
                {:else}
                  <button class="icon-btn" title="Edit" on:click={() => startEdit(p)}>✎</button>
                  <button class="icon-btn" title="Remove" on:click={() => deletingId = p.id}>🗑</button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if $serverRunning}
          <div class="running-note">Stop the running server before switching profiles.</div>
        {/if}
        {#if error}
          <div class="switcher-error">{error}</div>
        {/if}

        <button class="add-row" on:click={startAdd}>+ Add Server Profile</button>
      {:else}
        <div class="form">
          <label class="form-label" for="profile-name">Name</label>
          <input id="profile-name" type="text" bind:value={formName} placeholder="e.g. Master, TahitiRepro, 1.53 Test Bed">

          <label class="form-label" for="profile-exe">Server Executable</label>
          <div class="exe-row">
            <input id="profile-exe" type="text" bind:value={formExe} placeholder="Path to MHServerEmu.exe" readonly>
            <button class="btn-browse" on:click={browseExe}>Browse</button>
          </div>

          {#if error}
            <div class="switcher-error">{error}</div>
          {/if}

          <div class="form-actions">
            <button class="btn-cancel" on:click={() => { mode = 'list'; error = '' }}>Cancel</button>
            <button class="btn-save" on:click={saveForm}>{editingId ? 'Save' : 'Add'}</button>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .profile-switcher {
    position: relative;
    margin-left: 16px;
  }

  .switcher-btn {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 10px;
    background: var(--bg-2);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-1);
    cursor: pointer;
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    transition: all 0.12s;
    max-width: 280px;
  }
  .switcher-btn:hover {
    background: var(--bg-3);
    border-color: var(--border-lit);
    color: var(--text-0);
  }
  .switcher-btn svg {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
    color: var(--accent);
  }
  .switcher-btn .chevron {
    width: 11px;
    height: 11px;
    color: var(--text-3);
  }

  .switcher-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 180px;
  }

  .running-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #2ecc71;
    box-shadow: 0 0 5px #2ecc71;
    flex-shrink: 0;
  }

  .dropdown {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    width: 300px;
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    z-index: 200;
    padding: 8px;
  }

  .empty-note {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-2);
    padding: 8px 6px;
    line-height: 1.5;
  }

  .profile-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 240px;
    overflow-y: auto;
  }

  .profile-row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--radius-sm);
  }
  .profile-row:hover { background: var(--bg-3); }
  .profile-row.active { background: var(--accent-glow); }

  .profile-select {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    background: none;
    border: none;
    color: var(--text-1);
    cursor: pointer;
    text-align: left;
    font-family: var(--font-body);
    font-size: 12px;
    min-width: 0;
  }
  .profile-row.active .profile-select { color: var(--accent-bright); }

  .profile-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--border-lit);
    flex-shrink: 0;
  }
  .profile-dot.on {
    background: var(--accent-bright);
    box-shadow: 0 0 5px var(--accent-bright);
  }

  .icon-btn {
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-3);
    cursor: pointer;
    border-radius: var(--radius-sm);
    font-size: 11px;
  }
  .icon-btn:hover { color: var(--text-0); background: var(--bg-3); }
  .icon-btn.confirm {
    width: auto;
    padding: 0 8px;
    color: #e74c3c;
    font-size: 10px;
    font-weight: 600;
  }

  .running-note {
    font-family: var(--font-body);
    font-size: 11px;
    color: #e0a030;
    padding: 6px;
  }

  .switcher-error {
    font-family: var(--font-body);
    font-size: 11px;
    color: #e74c3c;
    padding: 6px;
  }

  .add-row {
    width: 100%;
    margin-top: 4px;
    padding: 8px;
    background: none;
    border: 1px dashed var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-2);
    cursor: pointer;
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    transition: all 0.12s;
  }
  .add-row:hover {
    color: var(--accent-bright);
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .form-label {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-2);
    margin-top: 6px;
  }
  .form-label:first-child { margin-top: 0; }

  .form input {
    background: var(--bg-1);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 8px;
  }
  .form input:focus {
    outline: none;
    border-color: var(--accent-dim);
  }

  .exe-row {
    display: flex;
    gap: 6px;
  }
  .exe-row input { flex: 1; min-width: 0; }

  .btn-browse {
    flex-shrink: 0;
    padding: 0 10px;
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-1);
    cursor: pointer;
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
  }
  .btn-browse:hover { color: var(--text-0); border-color: var(--border-lit); }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: 8px;
  }

  .btn-cancel, .btn-save {
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
  }
  .btn-cancel {
    background: none;
    border: 1px solid var(--border-mid);
    color: var(--text-2);
  }
  .btn-cancel:hover { color: var(--text-0); border-color: var(--border-lit); }
  .btn-save {
    background: var(--accent);
    border: 1px solid var(--accent);
    color: #fff;
  }
  .btn-save:hover { filter: brightness(1.1); }
</style>
