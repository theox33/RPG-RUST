.PHONY: all pdf clean open help

all: pdf

pdf: rapport.md
	@command -v pandoc >/dev/null 2>&1 || { echo "pandoc est requis. Installez pandoc et retentez."; exit 1; }
	@command -v xelatex >/dev/null 2>&1 || { echo "Remarque: xelatex non trouvé. pandoc utilisera le moteur par défaut si disponible."; }
	pandoc "rapport.md" -o "rapport.pdf" --pdf-engine=xelatex -V geometry:margin=1in --standalone

clean:
	rm -f rapport.pdf

open: pdf
	@command -v xdg-open >/dev/null 2>&1 && xdg-open rapport.pdf || echo "Ouvrez 'rapport.pdf' manuellement si nécessaire"

help:
	@echo "Usage: make [all|pdf|clean|open]"
