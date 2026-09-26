<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.md">English</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/si-jam-sessions/readme.png" alt="si-jam-sessions" width="400">
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/si-jam-sessions/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/si-jam-sessions/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
  <a href="https://mcp-tool-shop-org.github.io/si-jam-sessions/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page"></a>
</p>

# si-jam-sessions

**Un moteur musical dans lequel l’interprétation d’une IA peut être évaluée avec précision, rejouée exactement et utilisée pour un entraînement en toute tranquillité d’esprit.**

Jouez une phrase en rythme, et le moteur enregistre chaque note de l’enregistrement :

- le moment où elle a été jouée, en nombre entier d’échantillons ;
- la note de la partition à laquelle elle correspond.

Il évalue ensuite chaque note par rapport à la partition. Deux machines évaluant le même enregistrement sont d’accord jusqu’au dernier bit, car
la loi d’évaluation est déterministe, écrite en Rust et compilée en un seul fichier WebAssembly. La loi est déterministe, de sorte qu’un enregistrement
rejoué donne le même hachage sans appeler de modèle ; l’intégration continue le rejoue à chaque exécution.

`si-jam-sessions` est le complément de [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)
et un projet frère de [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine). Le modèle propose. Un
vérificateur, qui n’est pas le modèle, accepte ou refuse la proposition en donnant une raison, et la loi valide et hache
ce qu’elle accepte. Le modèle ne touche jamais directement la partition.

## Écoutez

Deux arrangements de *Battle Hymn of the Republic* sont joués côte à côte sur la
[page d’accueil](https://mcp-tool-shop-org.github.io/si-jam-sessions/), avec une vue de leurs différences.

- **La source :** la transcription de ce projet de l’édition anonyme de 1862 de Ditson.
- **Les arrangeurs :** glm-5.3 et kimi-k3, chacun travaillant à partir de cette source sur Ollama Cloud.
- **La licence :** les deux arrangements sont dédiés à la licence CC0 1.0. Chacun a passé le test de licence de la loi avec ses
preuves.
- **L’enregistrement :** chacun a été rendu à l’aide de la loi sur le
[Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) d’Alexander Holm
(CC BY 3.0).

| Arrangement | Durée | Notes validées par la loi | WAV non compressé (48 kHz, 32 bits en virgule flottante) |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav), SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav), SHA-256 `bf49d310…a017` |

Le rendu est déterministe : un deuxième rendu donne les mêmes octets. Le travail piano de l’intégration continue
a rendu chaque exemplaire en entier sous Linux, et les deux SHA-256 sont ceux de la publication : les mêmes sous
Windows et sous Linux. Après avoir récupéré le piano une fois, cette commande reproduit le premier :

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

Les hachages complets se trouvent dans les [notes de publication](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

## Ce qui le rend différent

- **L’enregistrement fait partie de la loi.** Le rythme, la hauteur et la vélocité sont des nombres entiers dans l’état haché, de sorte que « en retard de
45 ms » est un fait que le moteur peut prouver. La forme d’onde n’est qu’une présentation.
- **L’horloge n’attend jamais.** La loi avance d’un quantum fixe, que quelqu’un joue ou non. Un modèle planifie les phrases
avant un horizon de validation. Un plan tardif est refusé, et il n’est jamais autorisé à interrompre la musique.
- **Chaque morceau mérite sa place.** La musique n’entre que si elle est accompagnée de preuves :
- une composition du domaine public aux États-Unis et dans l’UE ;
- un arrangement du domaine public, ou un arrangement que ce projet a créé ;
- l’édition source et son année ;
- une licence dans le fichier qui correspond à sa réception.

Les sources inconnues, partagées, non commerciales et restreintes en matière d’IA sont refusées. Le matériel CC BY 4.0 se trouve dans sa
propre catégorie avec attribution.
- **Les données d’entraînement seront une impression de ce que la loi a validé.** Aucune base de données n’a encore été créée. Lorsqu’une base de données sera créée,
chaque ligne rejouera le même hachage, les divisions se feront par œuvre et seront fixées avant l’existence de toute ligne, et chaque
résultat revendiqué indiquera sa puissance statistique.

## Essayez-le

Vous avez besoin de Rust. Le dépôt fixe la version 1.98.1 dans `rust-toolchain.toml`, et `rustup` l’installe lors de la
première compilation.

- **Windows 10 et 11** exécutent tout, bien que l’entrée en direct n’ait pas encore été testée avec un véritable clavier MIDI.
- **Linux :** l’intégration continue compile et teste l’hôte et effectue des rendus avec celui-ci. La lecture via un périphérique audio Linux n’a pas été testée, et l’entrée en direct est uniquement disponible sur Windows pour le moment.
- **macOS** n’a pas été testé.

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece` sélectionne `battle-hymn-glm-5.3` (la valeur par défaut), `battle-hymn-kimi-k3` ou `entertainer`, le premier
morceau de test de l’étape 1.
- `notes <out.json>` écrit les notes que la loi valide. La page d’accueil extrait les données de ces fichiers.
- `host help` liste toutes les commandes.

Utilisez une sortie filaire pour `jam`. Le délai d’une sortie Bluetooth est supérieur aux 100 ms autorisés par la loi pour la livraison.

## Faire confiance au modèle

- **Données utilisées :**
- les fichiers de partition dans `scores/` ;
- les échantillons de piano dans un cache par utilisateur ;
- les sorties audio et les entrées MIDI que vous choisissez ;
- les fichiers que vous lui demandez d’écrire.
- **Données non utilisées :** tout ce qui se trouve en dehors de ces chemins. Il n’y a pas de comptes, pas d’identifiants et pas de télémétrie.
- **Réseau :** seul `fetch-piano` l’utilise.
- Il télécharge une archive à partir d’une adresse fixe.
- Il vérifie l’archive par rapport à un hachage SHA-256 fixe avant de décompresser quoi que ce soit.
- Il refuse les liens et les chemins qui sortent de son répertoire.
- **Autorisations :** un compte d’utilisateur ordinaire. Rien n’a besoin de droits d’administrateur.
- **La loi** n’effectue aucune opération d’E/S. L’intégration continue vérifie que son module WebAssembly n’importe rien.

Pour signaler une vulnérabilité, consultez [`SECURITY.md`](SECURITY.md).

## Où en est-il ?

- **La première étape est terminée :** la loi, l’ingestion et la provenance, le hachage doré et l’hôte. Le hachage doré est identique nativement sur x86_64 et ARM64, ainsi qu’en WebAssembly sous V8, SpiderMonkey et JavaScriptCore.
- **La conception** est finalisée dans [`docs/PHASE-0.md`](docs/PHASE-0.md).
- **La passation** dans [`docs/HANDOFF.md`](docs/HANDOFF.md) couvre ce qui est en cours, ce qui a été décidé et pourquoi, et ce qui va suivre.
- **Examen :** avant qu’une modification du code ne soit intégrée, deux modèles l’examinent sur Ollama Cloud. Chacun provient d’une famille différente de celle de l’auteur, et chacun ignore l’examen de l’autre.

## Licence

- **Code :** MIT (voir [`LICENSE`](LICENSE)).
- **Les deux arrangements de Battle Hymn :** CC0 1.0.
- **Les échantillons de piano :** CC BY 3.0 (Alexander Holm). `fetch-piano` les télécharge, et ils ne sont jamais enregistrés.
- **Ensembles de données :** chacun est assorti de sa propre licence sur sa propre fiche.

---

Créé par <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a>
