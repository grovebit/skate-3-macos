"""Convert your own Skate 3 (Xbox 360) for the engine, from its extracted game
folder or its .iso disc image. An existing installation is refreshed: only
asset groups whose pipeline changed are rebuilt, and player settings are kept.
"""
from pathlib import Path
import argparse,shutil,subprocess,sys,tempfile
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from tools import xiso
from tools.asset_pipeline.customiser_setup import install
from tools.asset_pipeline.setup_state import source_directory
from tools.asset_pipeline.versions import installed


def prepare(game,selected,output,game_exe,audio,report):
    game=source_directory(game,require_core=False)
    # The pipeline matches file names case-sensitively, even on a
    # case-insensitive APFS volume, and a miss silently drops content.
    names=[p.name for p in (game/'data/content').glob('*')]
    exact=sum(n.startswith('worldDIST_') and n.endswith('.big') for n in names)
    if exact!=sum(n.lower().startswith('worlddist_') and n.lower().endswith('.big') for n in names):
        raise RuntimeError('Map archives use unexpected casing; expected data/content/worldDIST_<Name>.big')
    stage=install(game,output,game_exe,report,refresh=installed(output) is not None,selected=selected)
    if audio and not audio.exists():
        decoder=shutil.which('vgmstream-cli')
        if decoder is None:
            report('Skipping the original skating sounds: they need vgmstream (brew install vgmstream)')
        else:
            from tools.audio.prepare_skating_audio import prepare as prepare_audio
            try:
                prepare_audio(game,audio,decoder)
            except (OSError,RuntimeError,ValueError,KeyError,subprocess.TimeoutExpired) as error:
                report(f'The original skating sounds could not be prepared: {error}')
    return stage


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game-root',type=Path,required=True,help='game folder, its default.xex, or an .iso image')
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--game-exe',type=Path,required=True)
    parser.add_argument('--skating-audio',type=Path,help='also decode the original skating sounds here once')
    args=parser.parse_args()
    report=lambda text:print(text,flush=True)
    output=args.output.resolve()
    output.mkdir(parents=True,exist_ok=True)
    try:
        if args.game_root.suffix.lower()=='.iso':
            # Extract beside the output so the space comes from the same volume.
            with tempfile.TemporaryDirectory(prefix='.disc-',dir=output) as temporary:
                game=xiso.extract(args.game_root,Path(temporary)/'game',report)
                stage=prepare(game,args.game_root,output,args.game_exe.resolve(),args.skating_audio,report)
        else:
            stage=prepare(args.game_root,args.game_root,output,args.game_exe.resolve(),args.skating_audio,report)
    except RuntimeError as error:
        parser.exit(1,f'Setup failed: {error}\n')
    print(f'Installed {stage}')

if __name__=='__main__':main()
