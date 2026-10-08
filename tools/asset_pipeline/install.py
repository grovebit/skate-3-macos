"""Local owned-disc installation. No game content is downloaded or packaged."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,sys,time,uuid
from concurrent.futures import ThreadPoolExecutor,as_completed
from tools.owned_game.big import BigArchive

TOOLS=Path(__file__).resolve().parents[1]

def map_workers():
    return min(3,max(1,(os.cpu_count() or 1)//2))

def digest(path):
    with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def remove_intermediate(path,root):
    target=path.resolve();root=root.resolve()
    if target==root or not target.is_relative_to(root):
        raise RuntimeError('Refusing to remove a path outside conversion workspace')
    shutil.rmtree(target)

# A full conversion peaks at about 5 GB of intermediate files.
WORKSPACE_SPACE=16<<30

def workspace(base,size=0):
    """Temporary directory for intermediate files, deleted afterwards; size is
    extra space the caller needs, such as an extracted disc. It is on the system
    volume when that has room, since a data folder on an external drive can
    write many times slower than the internal disk; otherwise it is in base."""
    import tempfile
    root=tempfile.gettempdir()
    if shutil.disk_usage(root).free<size+WORKSPACE_SPACE:root=base
    return tempfile.TemporaryDirectory(prefix='.conversion-',dir=root)

_INSTALLATION_ID=re.compile(r'^[0-9a-f]{32}$')

def active_installation_id(base):
    marker=base/'installation.json'
    if not marker.is_file():
        return None
    try:
        directory=json.loads(marker.read_text(encoding='utf-8-sig')).get('directory')
    except (OSError,ValueError,TypeError):
        return None
    if not isinstance(directory,str) or not re.fullmatch(r'installations/[0-9a-f]{32}',directory):
        return None
    return directory.split('/',1)[1]

def setup_log_name(name):
    return (name == 'setup.log'
            or name.endswith('-conversion.log') or name.endswith('-load.log'))

def remove_setup_logs(stage, report=lambda _:None):
    for path in stage.iterdir():
        if path.is_file() and setup_log_name(path.name):
            report('Removing setup log '+path.name)
            path.unlink(missing_ok=True)

def remove_stale_installations(base, active_stage, report=lambda _:None):
    """Drop superseded installation trees after a successful publish."""
    base=base.resolve()
    installations=base/'installations'
    if not installations.is_dir():
        return
    active_id=active_installation_id(base)
    active_root=active_stage.resolve()
    if active_id is None or active_root!=(installations/active_id).resolve():
        return
    if not active_root.is_relative_to(base) or not active_root.is_dir():
        return
    for entry in installations.iterdir():
        if not entry.is_dir() or not _INSTALLATION_ID.fullmatch(entry.name):
            continue
        if entry.resolve()==active_root:
            continue
        report('Removing previous installation '+entry.name)
        remove_intermediate(entry,installations)

def run(args,log,report):
    with subprocess.Popen([str(a) for a in args],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,
                          text=True,encoding='utf-8',errors='replace') as process:
        for line in process.stdout:
            log.write(line);log.flush()
        if process.wait():raise RuntimeError('Conversion failed. See '+str(log.name))

def task(script,*args):
    return [sys.executable,str(script),*map(str,args)]

def extract(archive,destination,entries=None):
    data=BigArchive(archive)
    data.extract_entries(data.entries if entries is None else [e for e in data.entries if entries(e)],destination)
    return data

def convert_map(archive,work,maps,stage,game_exe,log,report):
    timings={};started=time.perf_counter()
    def finished(phase):
        nonlocal started
        now=time.perf_counter();timings[phase]=round(now-started,3);started=now
        report(f'{archive.stem}: {phase} {timings[phase]:.3f}s')
    map_tools=TOOLS/'vendor/university/tools/vanilla_map_extraction/tools'
    sys.path.insert(0,str(map_tools))
    from prepare_hawaiian_dream import prepare
    from prepare_university import EXCLUDED_NORMAL_TEXTURE_IDS
    from build_retail_collision_archive import build_archive
    from .map_writer import write as write_map, SpawnSelector
    district=archive.stem.removeprefix('world')
    label=district.removeprefix('DIST_')
    district_work=work/district
    extract(archive,district_work/'raw')
    finished('extract')
    stream=district_work/'raw/data/content/world/stream'/district
    if not stream.is_dir():raise RuntimeError('Missing district stream '+str(stream))
    spawn=SpawnSelector(district)
    manifest_path=prepare(stream_directory=stream,output_root=district_work/'intermediate',
        utt_root=TOOLS/'vendor/utt',district_name=district,map_name=label,
        package_name='Skate 3 owned disc',cache_format='skate3-rust-map-v1',
        # Smaller parks keep their textures in Pres rather than a Tex stream.
        texture_stream_names=('Tex',) if any(stream.glob('cTex_*.xsf')) else (),
        excluded_normal_texture_ids=EXCLUDED_NORMAL_TEXTURE_IDS,raw_texture_cache=True,
        collision_consumer=spawn.consider,
        # Model/texture RX2 copies are unused by the direct writer and were
        # deleted after conversion. Keep simulation and irradiance sources.
        write_render_sources=False)
    finished('prepare')
    collision=district_work/'collision.rwcmset'
    build_archive(manifest_path,collision)
    finished('collision_archive')
    final=maps/(label+'.skate')
    write_map(manifest_path,final,collision,report,prepared_spawn=spawn.result(label))
    finished('write_map')
    from .dynamic_props import export as write_props
    caches=list((work/'dmo/cache').glob('DMO_*'))
    from .optional_content import CONTENT_ERRORS, note
    props=stage/'assets/private/native-props'/(label+'.skate')
    try:
        if not caches:raise RuntimeError('Movable-object source catalog is unavailable')
        placed, unresolved=write_props(manifest_path,caches,props,catalog_path=work/'dmo/catalog.json')
        (stage/'assets/private/native-props'/(label+'-availability.json')).unlink(missing_ok=True)
    except CONTENT_ERRORS as error:
        if props.is_dir():remove_intermediate(props,stage)
        note(stage/'assets/private/native-props'/(label+'-availability.json'),label+' movable props',error,report=report)
        placed,unresolved=0,0
    finished('props')
    report(f'{label}: placed {placed} authored DMO instances, {unresolved} unresolved templates')
    report('Checking converted map: '+label)
    run([game_exe,'--assets',stage/'assets','--map',final,'--check-assets'],log,report)
    finished('validate')
    entry={'name':label,'path':'maps/'+final.name,'sha256':digest(final)}
    remove_intermediate(district_work,work)
    finished('hash_and_cleanup')
    entry['phase_seconds']=timings
    return entry


def install(game,base,game_exe,report,refresh=False,finalize=None,source=None):
    from .setup_state import setup_lock
    with setup_lock(base):
        return _install(game,base,game_exe,report,refresh,finalize,source)


def _install(game,base,game_exe,report,refresh=False,finalize=None,source=None):
    from .versions import fingerprints, changed_groups, installed, GROUPS
    from .group_receipts import damaged, record
    from .setup_state import atomic_json, source_directory
    from . import asset_exports as exports
    target_versions=fingerprints()
    previous=installed(base) if refresh else None
    groups=changed_groups(previous[1].get('pipelines',{}),target_versions) if previous else set(GROUPS)
    # What the player selected (a folder or disc image), recorded for refreshes.
    source=str(Path(source or game).resolve())
    game_root=source_directory(game, require_core=not previous)
    source_hash=digest(game_root/'default.xex')
    if previous and previous[1].get('source_hash',source_hash)!=source_hash:
        raise RuntimeError('Select the same Xbox game edition used to set up this copy')
    base=base.resolve();base.mkdir(parents=True,exist_ok=True)
    if previous:groups.update(damaged(*previous, exclude=groups))
    def outputs(stage):
        saved=previous[1].get('outputs',{}) if previous else {}
        return {g:saved[g] if g not in groups and g in saved else record(stage,g) for g in GROUPS}
    if not groups:
        with (base/'refresh-validation.log').open('w',encoding='utf-8') as log:
            run([game_exe,'--assets',previous[0]/'assets','--test-world','--check-assets'],log,report)
        if finalize:finalize(previous[0])
        from .optional_content import summary
        summary(previous[0])
        atomic_json(base/'installation.json', {**previous[1], 'pipelines':target_versions,
                    'outputs':outputs(previous[0]), 'source':source})
        remove_setup_logs(previous[0],report)
        remove_stale_installations(base,previous[0],report)
        report('Game assets are current')
        return previous[0]
    install_id=uuid.uuid4().hex
    stage=base/'installations'/install_id
    stage.mkdir(parents=True)
    if previous:
        report('Preparing an asset update; keeping the previous installation until it succeeds')
        immutable_maps = ({(previous[0]/item['path']).resolve() for item in
                           json.loads((previous[0]/'maps.json').read_text())}
                          if 'maps' not in groups else set())
        from .customiser_cache import SOURCES as character_stages
        sets=previous[0]/'assets/private/customisation/sets'
        immutable_sets=[p.resolve() for p in sets.glob('*') if p.is_dir()
                        and all((p/(name+'-complete.json')).is_file() for name in character_stages)]
        for entry in previous[0].iterdir():
            if entry.name=='setup-report.json' or setup_log_name(entry.name):continue
            # Rebuilt maps are written fresh; a retained old map is copied below.
            if entry.name=='maps' and 'maps' in groups:continue
            # Unchanged maps and immutable character generations share storage.
            # Mutable user data and rebuilt outputs get independent files.
            def copy_map(src,dst):
                if Path(src).resolve() in immutable_maps:
                    try:os.link(src,dst)
                    except OSError:shutil.copy2(src,dst)
                else:shutil.copy2(src,dst)
                return dst
            def copy_asset(src,dst):
                if any(Path(src).resolve().is_relative_to(root) for root in immutable_sets):
                    try:os.link(src,dst)
                    except OSError:shutil.copy2(src,dst)
                else:shutil.copy2(src,dst)
                return dst
            def ignore_rebuilt_assets(directory, names):
                # These raw inputs are owned by the converters. A rebuilding
                # group must extract into an empty cache, not overwrite files
                # copied from the previous installation. Leave that live copy
                # and all user settings/custom models untouched.
                source_directory = Path(directory).resolve()
                private_source = (previous[0]/'assets/private').resolve()
                if 'core' in groups and source_directory == private_source:
                    return {'stock'} & set(names)
                if 'character' in groups and source_directory == private_source/'stock/data/content':
                    return {'createacharacter'} & set(names)
                return set()
            if entry.is_dir():
                shutil.copytree(entry,stage/entry.name,
                    ignore=ignore_rebuilt_assets if entry.name=='assets' else None,
                    copy_function=copy_map if entry.name=='maps'
                    else copy_asset if entry.name=='assets' else shutil.copy2)
            else:shutil.copy2(entry,stage/entry.name)
    private=stage/'assets/private';private.mkdir(parents=True,exist_ok=True)
    maps=stage/'maps';maps.mkdir(exist_ok=True)
    with (stage/'setup.log').open('w',encoding='utf-8') as log, workspace(stage) as work:
        work=Path(work)
        required_files=['default.xex']
        if 'core' in groups:required_files += ['data/big/miscload.big','data/big/miscboot.big','data/big/db.big']
        if 'character' in groups:required_files += ['data/content/createacharacter.big']
        for required in required_files:
            if not (game_root/required).is_file():raise RuntimeError('This is not a supported Skate 3 disc: missing '+required)
        stock=private/'stock'
        if 'core' in groups:
            converted=exports.core(game_root,stage,work,report,log)
        else:
            converted=json.loads((stock/'skater-collections.json').read_text(encoding='utf-8'))
        if 'hud' in groups:
            exports.hud(game_root,stage,work,report,log)
        if 'character' in groups:
            exports.character(game_root,stage,work,report,log,converted)
        if 'environment' in groups:
            exports.environment(game_root,stage,work,report,log,converted)
        if 'maps' in groups:
            report('Preparing authored movable-object models')
            from .dynamic_props import prepare_catalog
            from .optional_content import CONTENT_ERRORS, note
            try:
                prepare_catalog(game_root,work/'dmo')
                (private/'native-props/props-availability.json').unlink(missing_ok=True)
            except CONTENT_ERRORS as error:
                if (work/'dmo').exists():remove_intermediate(work/'dmo',work)
                note(private/'native-props/props-availability.json','Movable props',error,report=report)
        report('Validating skater, input and animation data')
        run([game_exe,'--assets',stage/'assets','--test-world','--check-assets'],log,report)
        if 'maps' in groups:
            archives=list((game_root/'data/content').glob('worldDIST_*.big'))
            archives.sort(key=lambda p:(p.stem!='worldDIST_University',p.name.lower()))
            workers=map_workers()
            report(f'Converting {len(archives)} maps with {workers} workers')
            def map_job(archive):
                result=work/(archive.stem+'.json')
                with (stage/(archive.stem+'-conversion.log')).open('w',encoding='utf-8') as map_log:
                    run(task(TOOLS/'asset_pipeline/map_job.py','--archive',archive,'--stage',stage,
                             '--work',work,'--game-exe',game_exe,'--result',result),map_log,report)
                return json.loads(result.read_text(encoding='utf-8'))
            completed={}
            with ThreadPoolExecutor(max_workers=workers) as pool:
                # Start the expensive districts together so one does not
                # remain queued behind a string of small parks.
                futures={pool.submit(map_job,a):a for a in sorted(archives,key=lambda p:-p.stat().st_size)}
                for future in as_completed(futures):
                    archive=futures[future]
                    try:completed[archive.name]=future.result()
                    except CONTENT_ERRORS as error:
                        label=archive.stem.removeprefix('worldDIST_')
                        (maps/(label+'.skate')).unlink(missing_ok=True)
                        props=private/'native-props'/(label+'.skate')
                        if props.is_dir():remove_intermediate(props,private)
                        note(private/'map-status'/(label+'-availability.json'),label,error,report=report)
                        continue
                    (private/'map-status'/(completed[archive.name]['name']+'-availability.json')).unlink(missing_ok=True)
                    report(f"Converted {len(completed)}/{len(archives)} maps: {completed[archive.name]['name']}")
            catalog=[completed[a.name] for a in archives if a.name in completed]
            if previous:
                # An absent/failed source district may still have a usable old
                # converted copy. Validate its bytes AND load with this engine.
                for old in json.loads((previous[0]/'maps.json').read_text()):
                    if any(item['path']==old['path'] for item in catalog):continue
                    src=(previous[0]/old['path']).resolve()
                    if not src.is_relative_to((previous[0]/'maps').resolve()):raise ValueError('Invalid old map path')
                    if not src.is_file() or digest(src)!=old.get('sha256'):continue
                    try:run([game_exe,'--assets',stage/'assets','--map',src,'--check-assets'],log,report)
                    except CONTENT_ERRORS:continue
                    target=stage/old['path'];target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,target)
                    if src.with_suffix('.irradiance').is_file():shutil.copy2(src.with_suffix('.irradiance'),target.with_suffix('.irradiance'))
                    catalog.append(old)
                    note(private/'map-status'/(old['name']+'-availability.json'),old['name'],
                         RuntimeError('New source map unavailable; previous map passed validation'),retained=True,report=report)
            if not catalog:raise RuntimeError('No playable map could be prepared or recovered. Restore at least one worldDIST_*.big archive beside default.xex and retry; the previous installation has been kept.')
        report('Validating installed runtime inputs')
        run([game_exe,'--assets',stage/'assets','--test-world','--check-assets'],log,report)
        settings=stage/'settings';settings.mkdir(exist_ok=True)
        if not previous:
            (settings/'default-map.json').write_text(json.dumps(next((m['path'] for m in catalog if m['name']=='University'),catalog[0]['path'])),encoding='utf-8')
        if 'maps' in groups:
            (stage/'maps.json').write_text(json.dumps(catalog,indent=2),encoding='utf-8')
            selected=settings/'default-map.json'
            if selected.is_file() and not (stage/json.loads(selected.read_text())).is_file():
                selected.write_text(json.dumps(catalog[0]['path']))
        if finalize:finalize(stage)
        from .optional_content import summary
        warnings=summary(stage)
    # Publish after core validation and all optional outcomes have been recorded.
    marker=base/'installation.json.new'
    marker.write_text(json.dumps({'version':1,'directory':'installations/'+install_id,'source':source,'source_hash':source_hash,'pipelines':target_versions,'outputs':outputs(stage)}),encoding='utf-8')
    marker.replace(base/'installation.json')
    remove_setup_logs(stage,report)
    remove_stale_installations(base,stage,report)
    report(f'Setup complete ({len(warnings)} unavailable/retained components; see setup-report.json)' if warnings else 'Setup complete')
    return stage
